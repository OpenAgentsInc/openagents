# Build 22 verification: clearer credit

The checks for TestFlight build 22 on 2026-09-29: the phone fixes for
[#9948](https://github.com/OpenAgentsInc/openagents/issues/9948) and
[#9949](https://github.com/OpenAgentsInc/openagents/issues/9949)
(`69587cc286`), the Gym news latency fix
([#9950](https://github.com/OpenAgentsInc/openagents/issues/9950),
`dbad257c51`), and the first-run flow, against the live services.
Screenshots are in [`2026-09-29-build-22/`](2026-09-29-build-22/). Build 21's
run is [the build 21 verification](2026-09-29-build-21-verification.md).

## What ran, and against what

- **Live services.** The chat worker on `oa-coder-worker-1` (release
  `dbad257c51` at the start, then `c975123ca2` and `8fdad245cc`, deployed
  during this run; see [the chat worker runbook](../../deployment/chat-worker.md)),
  the hosted eval runner on `coderos-4080` (`c815433291`), the XP referee
  timer on the same host, and `relay.openagents.com`.
- **Android.** The `oa_chat1st` emulator (arm64) with a debug APK built from
  `origin/main` (`cffcc6120d`), then rebuilt with each fix below. Two fresh
  installs, each with its own device and world keys: Trainer 4GS and, on the
  final APK, Trainer WEL.
- **Other trainers.** Two Project map results for 4GS to check were made
  with `crates/eval-runner/examples/trainer.rs` from fresh keys, through the
  hosted runner like a phone: `be8c12f735e6…` and `d37a622f0d77…`, both 5 of
  6 against 2 of 6.
- **iOS.** Not run; see [iOS](#ios).

## Results

| Check | Platform | Result | Evidence |
| --- | --- | --- | --- |
| `FLOW-01` first run: Choose Coder → end card → first-run chat → **START THE TEST** | Android | Pass: 3 taps, 17 s from the first tap on the first install (4GS), 6 s by automation on the final APK (WEL) | `android-FLOW-01-1` to `-4` |
| `FLOW-01` result → **Add to the Gym** → menu | Android | Pass: result in 26 s (4GS) and 43 s (WEL) on the runner, 2 of 6 → 5 of 6, Better; published `af1622fb4151…` (4GS) and `57b4238ea3a6…` (WEL); the sheet's button says **TO THE MENU** | `android-FLOW-01-5` to `-8` |
| Check a result, first offer | Android | **Failed, fixed** ([#9951](https://github.com/OpenAgentsInc/openagents/issues/9951)): offered a chat-made tool's result (Changelog writer, "3 of 6 tests instead of 3"); **RUN THE CHECK** answered "Our test computers don't run this tool or test set" (runner: `not_admitted: the hosted runner tests catalog tools and tools made in chat only`) | `android-BUG-9951-*` |
| Check a result, second offer | Android | **Failed, fixed** ([#9952](https://github.com/OpenAgentsInc/openagents/issues/9952)): offered build 21's result `195ca9e2…`; the check confirmed it (`16dcb6542461…`) and promised +50 XP, and the referee refused it: `not a check of this result: another suite, subject, or subject lock`. The subject lock names the runner's binary, so every runner redeploy leaves older results uncheckable for credit | `android-BUG-9952-*` |
| `FLOW-08`/`FLOW-10` after the fixes | Android | Pass: offered `d37a622f…` (+50 XP); check `74a41204d0d1…`; the referee signed 2 awards in its 07:04:55 pass; the menu showed 50 / 100 XP and "+50 XP" | `android-9948-1` to `-3` |
| **#9948** a second check of the same test set | Android | Pass: the next offer (`be8c12f7…`, the same Project map test set) has no +XP badge and says "You already earned XP for checking this test set. This check earns no more."; its result card and **Add to the Gym** sheet say the same | `android-9948-4` to `-6` |
| **#9948** on Profile | Android | **Failed, fixed** ([#9953](https://github.com/OpenAgentsInc/openagents/issues/9953)): the second check still showed "Your check holds up. XP is on its way", and the next step said "a check confirmed your result. XP is on its way." Now it's "A check you added", and the next step is "check someone else's result for more XP" | `android-BUG-9953-*`, `android-9949-2-profile-final.png` |
| **#9949** Profile's Your results | Android | Pass: with one result and three checks, Your results lists only the Project map run (2 of 6 → 5 of 6, Better); the checks show under What you made | `android-9949-1`, `-2` |
| The XP bar | Android | **Failed, fixed** ([#9954](https://github.com/OpenAgentsInc/openagents/issues/9954)): "50 / 100 XP" over an empty bar on the menu and Profile (build 21's screenshots show the same). Now half full | `android-BUG-9954-xp-bar-empty.png`, `android-9948-3-menu-credit.png` |
| "What's new in the Gym?" | Android | Pass: the lead line and the news card with the records at 1.56 s and 1.62 s after the turn started (worker log), the model's first words at 2.37 s and 2.39 s, done at 3.3 s and 3.4 s; `4 cited, 0 invented, banned [], 0 raw ids` both times; no ids or banned words on screen | `android-news-1.png`, `android-news-2.png` |
| "Who are you?" | Android | Pass: the prepared answer on screen within 1 s of sending, with "Prepared answer", follow-up chips, and **Wrong answer** | `android-who-are-you.png` |

The news reply's first words (the bank's lead line, sent with the card)
came at 1.6 s in both runs here, against the median 1.15 s over ten runs
in [the latency measurement](../../coder/measurements/2026-09-29-gym-news-latency.md);
the relevance judgment before them is most of it. That's inside
`LF-G4`'s 2 s for first words, and the 8 to 12 s of build 21 is gone.

## Bugs found and fixed

| Bug | Fix |
| --- | --- |
| [#9951](https://github.com/OpenAgentsInc/openagents/issues/9951) Check a result offered a chat-made tool's result, which the hosted runner can't rerun (the skill stays on its maker's phone) | The chat worker offers only results of our catalog tools; a check card for an even result says "Coder passed 3 of 6 tests with X and without it" instead of "pass 3 of 6 tests instead of 3"; `c975123ca2`, deployed |
| [#9952](https://github.com/OpenAgentsInc/openagents/issues/9952) Check a result offered results from before the runner's last redeploy, whose checks the referee refuses | Each result is `current` when it ran under the newest subject lock read for its test set and subject, and only current results are offered; `8fdad245cc`, deployed. A redeploy with no run since still leaves the newest lock stale until someone runs the test set |
| [#9953](https://github.com/OpenAgentsInc/openagents/issues/9953) Profile called a second check on a paid test set pending | `xp_ledger::eval::made` marks another result or check of the same kind on an awarded test set no-credit; `ec12831e40` |
| [#9954](https://github.com/OpenAgentsInc/openagents/issues/9954) The Android XP bar never filled | The fill's share is a layout weight; `9989ec27b2` |

## iOS

The host's iOS Simulator was still unusable. The scratch device
`oa-verify-21` (`E524E630…`) showed as Booted, but `xcrun simctl
bootstatus -b` didn't finish within 20 minutes (it stayed in first-boot data
migration), and launching Settings on it timed out twice. `dasd` was no
longer pegged, and the data volume was 88 % full. No iOS build was
installed; the device was deleted afterwards, and the owner's device was
never used. The same Rust core ran every check above on Android; the
iPhone walk of build 22 (`FLOW-01`, the second check, Profile, and the XP
bar) is an owner step in `NEEDS_OWNER.md`.

## Test sweep

At `origin/main` before the fixes (`cffcc6120d`): `openagents-mobile`
passed 189 (20 ignored), `coder --lib` 735 (3 ignored), and `cargo clippy
--all-targets -D warnings` on `openagents-mobile` was clean. With the fixes:
`openagents-mobile` 190 (20 ignored, clippy clean), `coder --lib` 736 and
the `coder-worker` binary's 35 (clippy clean), `xp-ledger` 37 (clippy
clean), `verse` 335; `cargo fmt --check` clean; Android lint and unit tests
(`build-openagents-android.sh check`) passed.

## Build 22

Archived with `bins/openagents-ios/build.sh archive` from `a9ba87d828` (a
clean checkout of `main`; `archive-source.status` empty; the archive says
1.0.0 (22)), default archive settings, and uploaded with `build.sh upload`
at 12:18 UTC. App Store Connect (filtered by pre-release version 1.0.0 and
build 22): uploaded 2026-09-29T12:19:13Z, processing state `VALID`,
internal state `IN_BETA_TESTING`.

The chat worker then moved to `a9ba87d828`, which carries build 22's
changelog for Gym news; `live_basic_coder_streams_a_reply` passed on it
with first words at 0.68 s.
