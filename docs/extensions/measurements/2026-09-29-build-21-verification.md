# Build 21 verification: evals in chat, end to end

The end-to-end run, lo-fi round, and test sweep for
[#9941](https://github.com/OpenAgentsInc/openagents/issues/9941) (epic
[#9931](https://github.com/OpenAgentsInc/openagents/issues/9931)) before
TestFlight build 21, on 2026-09-29. Screenshots are in
[`2026-09-29-build-21/`](2026-09-29-build-21/).

## What ran, and against what

- **Live services.** The chat worker on `oa-coder-worker-1` (releases
  `0546032e17`, then `55591061c0` and `d0a053650d`, deployed during this run;
  see [the chat worker runbook](../../deployment/chat-worker.md)), the hosted
  eval runner on `coderos-4080` (`0b39640d66`, then `c815433291` with relay
  liveness; see [the runner runbook](../../deployment/eval-runner.md)), the XP
  referee timer, and `relay.openagents.com` (revision `00034-pit` from 09:07
  UTC).
- **iOS.** A scratch iPhone 17 Pro simulator (iOS 26.5), never the owner's
  `8010197D…` device, with a fresh install: Trainer PCX.
- **Android.** The `oa_chat1st` emulator (arm64), a debug APK of this change,
  fresh install: Trainer DJF.
- **Fresh keys.** Each install made its own device and world keys, so the 40
  messages a day quota was never reached. The chat worker's own live test
  (`live_basic_coder_streams_a_reply`) used a fresh key per message.

## Results

| Flow | Platform | Result | Evidence |
| --- | --- | --- | --- |
| `FLOW-01` first run: Choose Coder → end card → first-run chat → **START THE TEST** | iOS | Pass: 3 taps, 19 s from the first tap to the run starting | `ios-FLOW-01-1` to `-4` |
| `FLOW-01` result → **Add to the Gym** → menu | iOS | Pass: result in about 60 s, 2 of 6 → 5 of 6, Better; published `3189` `9dfc0bf0a4089fd312e6748f01c5a932ba9bfce458d3505cd1c3f801788b94bc` | `ios-FLOW-01-5` to `-9` |
| `FLOW-01` the same | Android | Pass: 3 taps, 19 s; result 2 of 6 → 5 of 6; published `195ca9e29065dbed61fbfbdcbd3fcb2c2d82fc787022fb3a2ad1e6b11436b989`; the added sheet's button says **TO THE MENU** | `android-FLOW-01-*` |
| `FLOW-08` check another trainer's result | iOS | **Failed, fixed**: "Check a result" got a "no records" model reply, and the typed request offered PCX its own result, whose check (`9503cf47…`) the referee refused | `ios-BUG-*` |
| `FLOW-08` after the fix | Android | Pass: DJF was offered PCX's result, not its own; confirmed it (5 of 6 vs 2 of 6); check `e00e8178b454728f15a2b7f9cf6b1fcb8a6efa0cc0f42296dd8470d2b4bf4210` | `android-FLOW-08-*` |
| `FLOW-10` credit | Android | Pass: the referee signed `3193` `2dc05529b654e84b…` (DJF, checker, +50) and `ea0fc40be512b0c7…` (PCX, evaluator, +25) within 3 minutes; DJF's menu showed 50 of 100 XP and "+50 XP", and Profile's **What you made** showed "Your check was confirmed +50 XP" | `android-FLOW-10-*` |
| `FLOW-03` kicking the tires | Android and live test | Pass after one fix: "Who are you?" and "What model is this?" are prepared answers at once; "What's the Gym?" was answered about OpenAI's Gym (fixed with the `gym.what` answer, now 0.70 s, prepared) | `android-FLOW-03-*` |
| `FLOW-09` what's new in the Gym | Android | Pass, slow: the news card and a grounded reply with no ids or banned words; first words at 12 s ([#9950](https://github.com/OpenAgentsInc/openagents/issues/9950)) | `android-FLOW-09-news.png` |
| `FLOW-07` make a tool by chatting | Android | Pass after one fix: "Help me make a tool that writes changelog entries" → the tool (a skill) → **Looks good** → our answer → 6 tests (2 where the tool stays out of the way) → checks → **TRY IT ONCE** (2 of 6 → 3 of 6) → **RUN THE FULL TEST SET** (3 of 6 → 3 of 6, No clear change) → **Add to the Gym**: `3184` test set `552101f25b9b073476bb1efd27427fda4e0c9c62d5a455f3f45a4cb122a5d9cb` and result `482daa98d60f82658c4802ea02236ee140ac266677a5f89f80f57ef27346b689`. The six-test draft's **Looks good** was out of reach until the card-scroll fix | `android-FLOW-07-*`, `android-BUG-draft-button-unreachable.png` |
| The Gym board in the Verse, and Compare notes | Android | Pass: the board lists both test sets and every result with its trainer, checks, and credit; Compare notes is off by default and says what it shares. Its jargon ("Evals", "eval results", "hosted run", "keys", "relay") was replaced | `android-verse-gym-board.png` |
| Wallet **Receive** and **Send** (no money moved) | Android | Pass: Receive shows a QR code and a request for any amount; Send shows Paste or scan, Paste, Scan, Continue | `android-wallet-*` |
| Keyboard dismissal | Android | Pass: a tap outside the composer puts the keyboard away | `android-keyboard-dismissed.png` |
| Relay liveness of the hosted runner | Host | Pass after a fix: the runner reconnected at 09:44 UTC only because the old relay instance served until its one-hour timeout; it now probes and renews like the chat worker (`c815433291`, deployed 09:45) | [runner runbook](../../deployment/eval-runner.md) |

The iOS simulator stopped launching any app at about 03:57 local time and
stayed that way through several resets, fresh devices, and restarts of the
simulator service. The host's data volume was 99 % full at the time (build
directories, since deleted). Settings and Contacts didn't launch either, so
this is the simulator host, not the app; the same Rust core ran every flow
on Android. The iOS walk of `FLOW-03`, `FLOW-07`, the Verse board, and the
Wallet on build 21 is an owner step in `NEEDS_OWNER.md`.

## Bugs found and fixed

| Bug | Fix |
| --- | --- |
| The menu's **Check a result** chip sent "Is there a result I can check?", which the router reads as `eval.check` at 0.46 to 0.54, so the model answered that it had no records | The chip sends "Find me a result to check" (`eval.check` 0.99); `aba0a02e94` |
| The chat worker holds no trainer key, so it offered a new trainer their own newest result to check; the referee refuses a self-check, so the promised XP never came | The request carries `skip` (the trainer's own results and those it checked); `aba0a02e94`, deployed |
| A check's **Add to the Gym** said "your check of A trainer's result" and "You earn XP when they confirm it" | Lowercase in a sentence, and check-specific XP lines; `aba0a02e94` |
| After the first result, the added sheet said BACK TO CHAT but went to the menu | TO THE MENU; `aba0a02e94` |
| "What's the Gym?" read `product.kb` near the floor: stale notes about the benchmark board, or OpenAI's Gym | Bank answer `gym.what`, rubric example, 4 labeled rows; held-out canned precision 100 % (50/50); `d0a053650d`, deployed |
| Answers and notes said tests in chat were "on their way" | Bank and notes updated; `978aff10b7`, `cf50a887ae`, deployed |
| Android: a draft card taller than half the screen couldn't be scrolled to **Looks good** (rebuilt every second) | Card views kept until their content changes; `2e3ef62372` |
| Android: a sheet's lone button sat under the navigation bar; a busy button read "ADDING… …" | Sheet insets and a single ellipsis; `2e3ef62372` |
| The Gym board showed "eval", "hosted", "keys", and "relay"; the menu said "Your work was checked" for a checker's own XP | Plain words on iOS, Android, and in `verse`; `1b1b20dab7` |
| The hosted runner had no relay liveness | `c815433291` |
| Flaky tests: Wasmtime's Mach-port trap thread aborted the coder test binary on SIGCHLD; the exit vault test matched `leaf` in base64 | `b72514f18f`, `b24c7b5030` |

Filed, not fixed in build 21:
[#9948](https://github.com/OpenAgentsInc/openagents/issues/9948) (a second
check of the same test set version still shows +50 XP),
[#9949](https://github.com/OpenAgentsInc/openagents/issues/9949) (Profile
lists a try and a check among Your results),
[#9950](https://github.com/OpenAgentsInc/openagents/issues/9950) (Gym news
first words at 8 to 12 s).

## Lo-fi round

- **Mockup** (`bins/openagents-mockup-ios`): `build.sh check` found no banned
  word on screen, and `build.sh test` passed `testFlow01FirstTimePlaytester`
  (3 taps and the relaunch resume), `testFlow02ReturningPlaytester`,
  `testFlow07MakeAToolInChat`, and `testFlow08CheckAResult` on a scratch
  simulator.
- **Real app, by automation** (no testers): the thresholds that automation can
  measure.

| Goal | Threshold | Measured |
| --- | --- | --- |
| `LF-G2` taps to the first run | 3 | 3 on iOS and Android |
| `LF-G2` time to the first run | under 2 minutes | 19 s from the first tap (the end card plays no film yet) |
| `LF-G4` prepared answers | under 1 s | 0.62 to 0.70 s ("Who are you?", "What model is this?", "What's the Gym?", "How do I earn XP from tests?") |
| `LF-G4` other first words | under 2 s | Fails for Gym news (12 s, #9950); the interview's steps 3 to 25 s |
| `LF-G8` banned words | none on a v1 screen | Fixed on the board and in the notes; the interview's model text can still say "operation check" (not on the list) |
| `LF-G9` make a test set | finishable, each step a tap | Finishable on Android after the scroll fix; each gate waited for a tap |
| `LF-G10` news, checks, credit | reachable | Reached; credit arrived within 3 minutes |

`LF-G1`, `LF-G3`, `LF-G5` to `LF-G7`, and the understanding questions need
testers; the round with people is the owner's step in `NEEDS_OWNER.md`.

## Test sweep

At `caf14e1a3b` plus this change's commits: `cargo test` for `coder`, `gym`,
`nostr`, `verse`, `ext-eval`, `knowledge`, `openagents-cli`, `microcoder`,
`xp-ledger`, `plugin`, and `eval-runner` passed 2,846 tests with 0 failures;
`openagents-mobile` passed 187 (20 ignored); `cargo clippy --all-targets -D
warnings` was clean for all of them; `cargo fmt --check` was clean in both
Cargo workspaces. `cargo test -p coder --lib` passed 5 times in a row after the
Wasmtime fix, and the exit vault module 100 times.

## Build 21

Archived with `bins/openagents-ios/build.sh archive` from `1b1b20dab7` (a
clean checkout of `main`; `archive-source.status` empty), default archive
settings, and uploaded with `build.sh upload` at 10:37 UTC. App Store Connect
(filtered by pre-release version 1.0.0 and build 21): uploaded
2026-09-29T10:38:41Z, processing state `VALID`, internal state
`IN_BETA_TESTING`.
