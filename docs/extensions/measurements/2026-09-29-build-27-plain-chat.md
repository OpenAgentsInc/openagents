# Build 27: a plain chat (2026-09-29)

The owner's direction on seeing build 26 on the simulator: remove the
person icon and the Cloud pill from the chat header (anchor them to
Account), remove the line under the header, remove the Wrong answer
button, and remove the "Prepared answer" note.

## What changed (`d65db8f209`)

- The chat header keeps the previous-chats button and the title. No
  Profile icon, no Cloud pill.
- **Profile** is a row under Account in both hosts (op `profile`,
  `Request::Profile` → `CoderTab::show_profile`), which shows the Profile
  sheet on the Chat tab and asks the host to switch to it, the way Train
  Coder does.
- Where a chat runs shows as a pill under the header (`coder-target-row`)
  only once a computer is enrolled; with none, every chat is the cloud
  and nothing is shown. The picker's rows and behavior are unchanged.
- The welcome line ("Ask us anything…") is gone.
- Replies carry no "Prepared answer" note, and the Wrong answer button,
  its confirm and send steps, the `wrong_answer` go, the
  `report_wrong_answer` op, and the playtest form for it are gone from
  Rust and both hosts. Report a problem (long press on the tab bar) is
  unchanged.

## Tests

`cargo test --manifest-path crates/openagents-mobile/Cargo.toml`: 193
passed, 20 ignored; clippy clean (the request enum's report form is now
boxed, which the variant-size lint asked for once the wrong-answer
variant left). The prepared-answer test now checks for no note and no
report control; the no-computer landing test checks for no target pill
and no welcome line.

## Simulator

`build.sh sim` on the iPhone 17 Pro simulator over build 26's install:
the chat opens with the plain header and no line under it.

## Build 27

Archived from `d65db8f209` (clean) with `build.sh archive`; the archive
says `com.openagents.app` 1.0.0 (27). Uploaded with `build.sh upload`
between 19:01 and 19:02 UTC ("Upload succeeded", "EXPORT SUCCEEDED").
App Store Connect processing state: pending at the time of this commit;
updated once the build shows VALID.
