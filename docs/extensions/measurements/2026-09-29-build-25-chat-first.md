# Build 25: chat first (2026-09-29)

Five issues landed on `main` between builds 24 and 25, all from the
owner's direction of 2026-09-29: land people in chat, apply the eval system
to the chat routes, say when a capability is missing, one vocabulary, and
prove the loop.

| Issue | Commit | What |
| --- | --- | --- |
| [#9957](https://github.com/OpenAgentsInc/openagents/issues/9957) | `06ee2fff22` | One vocabulary: people add capabilities, shipped as extensions, as programs, plugins, skills, or knowledge; glossary, NIPs, README, extension docs |
| [#9958](https://github.com/OpenAgentsInc/openagents/issues/9958) | `749c12f4be` | Chat first: a fresh install opens on a chat; the Gym intro is behind **Train Coder** (Verse board, Account); on-screen "tool" is now "capability"; build 25 changelog |
| [#9959](https://github.com/OpenAgentsInc/openagents/issues/9959) | `bc4a40b17e` | The chat router as a capability claim: NIP-EVAL record, `router-v1` gate, calibration (off), question-set and bank digests on the wire; live held-out measurement |
| [#9960](https://github.com/OpenAgentsInc/openagents/issues/9960) | `fc6c750dd2` | `chat-router-v3`: the admitted-capability set, the `capability` question, the `capability.missing` route and card; 100 % precision on the held-out split |
| [#9961](https://github.com/OpenAgentsInc/openagents/issues/9961) | `4467f24eb9`, `1c22654122` | Coder consumes `coder-defaults`; Project map validated on an independent second suite and adopted (release `680720dd100f`); Jev-probe packaged as a program subject |

The deck changes (`bec33dd15a`, `515f6ca256`, `375cef66ef`) are not in
the app.

## What the phone shows

- A fresh install opens the Chat tab on a chat with OpenAgents. No Choose
  Coder screen, no step counter, no auto-sent test, no Gym chips.
- The person icon in the chat header opens Profile. **Train Coder** on the
  Verse's results board and under Account opens the Gym intro (Choose
  Coder, Let's go, Start the test); **Not now** returns to the chat.
- A request the chat has no capability for gets a **NO CAPABILITY FOR THAT
  YET** card with **ADD A CAPABILITY** (or **SEE THE GYM**).
- Every Gym label says capability: "Test a capability", "Coder has Project
  map now", and the check card.
- Account, What's new: build 25 "Chat first".

## The chat worker

The card only arrives when the worker serves `chat-router-v3`, so the
worker was released as `375cef66ef` before the upload
([the deployment doc](../../deployment/chat-worker.md)). Live check from
this Mac through the production relay: "Book me a flight to Austin
tomorrow" answered in 0.71 s (first words 0.68 s) with route
`capability.missing`, answer `capability.missing@1`, and the card text;
"Who are you?" still answers from the bank.

## Tests

- `cargo test --manifest-path crates/openagents-mobile/Cargo.toml` on the
  merged `main` (`375cef66ef`): 193 passed, 20 ignored.
- Each issue's agent ran its own crates' tests and clippy before pushing;
  the issue comments list them.
- The Swift and Kotlin hosts were edited without a native build by the
  #9958 agent; the iOS archive below compiled them.

## Build 25

Archived from `375cef66ef` (clean; `project.yml` bumped to 25 in the same
commit as this record) with `bins/openagents-ios/build.sh archive`; the
archive's Info.plist says `com.openagents.app` 1.0.0 (25). Uploaded with
`build.sh upload` between 18:34 and 18:35 UTC ("Upload succeeded",
"EXPORT SUCCEEDED"). App Store Connect (filtered by pre-release version 1.0.0 and build 25)
shows it uploaded at 2026-09-29T11:35:58-07:00, processing state `VALID`.

## Not in this build

- Android was not built or uploaded.
- The Terminal-Bench harness's Jev-probe result is packaged as a subject
  but its Gym run was inconclusive (cost unknown in both arms); see
  [the first adoption record](2026-09-29-first-adoption.md).
- The eval runner on `coderos-4080` runs `4467f24eb9`; the log-first-read
  fix in `1c22654122` is not deployed.
