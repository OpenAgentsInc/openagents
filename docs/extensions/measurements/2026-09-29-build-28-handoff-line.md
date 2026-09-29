# Build 28: a handoff to a computer is one line (2026-09-29)

The owner tapped Run Coder on a computer from a chat on build 27 and got
"a big thing" that would not scroll.

## Cause

`CoderTab::run_coder` builds the task's prompt with `handoff`: the chat's
title, a marker line, and the conversation so far, newest turns kept,
within 16 KB. It then echoed that whole prompt as the task chat's pending
message until the computer's transcript arrived, and the computer's
transcript shows the same prompt again as its first user message. One
message of up to 16 KB in the transcript is the wall of text, and it
crowded the task's chat.

## Fix (`7fe0739b97`)

`basic_chats::handoff_summary` recognizes the marker line and returns
"Continued from the OpenAgents app: <title>". The pending echo (`sent`)
and the transcript's user rows (`draw`) show that line instead of the
prompt; the echo's own text is unchanged, so its match against the
transcript still settles. Tests: `a_handoff_reads_as_one_line_on_the_phone`
in `basic_chats`; `run_coder_starts_a_task_with_the_conversation` now
checks for the one line and for the absence of the pasted conversation.

Not reproduced on the simulator: it has no connected computer. The
synthetic-computer test covers the same view.

## Tests

`cargo test --manifest-path crates/openagents-mobile/Cargo.toml`: 194
passed, 20 ignored; clippy clean.

## Build 28

Archived from `7fe0739b97` (clean) with `build.sh archive`; the archive
says `com.openagents.app` 1.0.0 (28). Uploaded with `build.sh upload`
between 19:13 and 19:14 UTC ("Upload succeeded", "EXPORT SUCCEEDED").
App Store Connect (filtered by pre-release version 1.0.0 and build 28)
shows it uploaded at 2026-09-29T12:15:49-07:00, processing state `VALID`.
