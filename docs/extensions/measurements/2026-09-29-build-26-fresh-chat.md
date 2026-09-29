# Build 26: a fresh chat on upgrade (2026-09-29)

The owner opened build 25 over build 24 and saw the old "Test Project map
on Coder" chat with its result, and New chat did nothing.

## Cause

Build 24 left that phone's saved intro at `Chat` with the intro's talk
recorded and, since the opt-in did not exist yet, no Gym opt-in. Build
25's `CoderTab::refresh` (FLOW-01: "the first-run chat reopens where it
was") reopened that talk whenever no chat was open, on every frame,
without checking the opt-in. New chat and Back clear the open chat, so the
next frame reopened the intro's chat again.

## Fix (`b807d25954`)

The intro's chat reopens only for a person who opted into the Gym, once
per launch, and never after New chat or Back (`intro_reopened`). The
intro's chat stays in Previous chats as history. The regression test
`an_upgraded_phone_with_an_unfinished_intro_opens_on_a_fresh_chat` walks
the intro to its chat, clears the opt-in the way an upgrade does,
relaunches, and checks the app opens on a fresh chat with no card and no
intro message, that the drawer lists the intro's chat, and that New chat
does not reopen it.

## Tests

`cargo test --manifest-path crates/openagents-mobile/Cargo.toml`: 194
passed, 20 ignored; clippy clean; `cargo test -p coder gym` green.

## Simulator

`bins/openagents-ios/build.sh sim` on an iPhone 17 Pro simulator (the
first launch on that simulator, so a fresh install): the app opened on a
chat with OpenAgents, no Choose Coder screen, no Gym card.

## Build 26

Archived from `b807d25954` (clean) with `build.sh archive`; the archive
says `com.openagents.app` 1.0.0 (26). Uploaded with `build.sh upload`
between 18:49 and 18:50 UTC ("Upload succeeded", "EXPORT SUCCEEDED").
App Store Connect (filtered by pre-release version 1.0.0 and build 26)
shows it uploaded at 2026-09-29T11:51:14-07:00, processing state `VALID`.
