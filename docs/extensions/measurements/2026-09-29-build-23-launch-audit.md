# Build 23 launch audit (2026-09-29)

Issue [#9955](https://github.com/OpenAgentsInc/openagents/issues/9955). The
owner's directive: none of the static-screen work (the mockup app, fixtures,
wireframe-only screens) may be in the shipped app, and every visible element
must be live.

## Findings

| Where | What | Done |
| --- | --- | --- |
| `bins/openagents-mockup-ios` | Not linked, bundled, or referenced by the real app; no `MockData` use, no shared assets, only generic words in common | Nothing to do |
| Wireframe features (Rankings, Updates, bell, Coder row, CIN-01, season card) | No code in the real app; the menu has PROFILE and THE GYM IN THE VERSE, both live | Nothing to do |
| `crates/openagents-mobile/src/{chat,gym,wallet}_fixture.rs` | Compiled into release, switched off at run time only | Compiled only into debug builds |
| `app.rs` Computers fixture | `launch.computers_fixture` was honored by Rust in any build (only the Android host gated it) | Debug builds only |
| `verse.rs`, `android.rs` `gym_preview`, `results_base` | Synthetic Gym board and results mirror honored by Rust in any build | Forced off in release |
| `bins/openagents-ios/host/project.yml` | `conversation.json` fixture was in the App Store bundle | Copied only into debug, simulator, and bench builds |
| `NativeFixture.swift` | Fixture screen compiled into release (unreachable) | Compiled out of device release builds |
| `AppTabs.swift` `ComingSoonScreen` | Unused "Coming soon." screen | Removed |
| Account > Playtest card (iOS, Android) | Zeros while the playtest referee key is unpublished (`PLAYTEST_REFEREE = None`), not from any record | Shown only once awards are read |
| `gym.rs` refusal lines | "Our test computers aren't open yet" though the hosted runner is live | The real reason: the size limit (8 tests, 3 runs) or a lost draft |
| Report a problem | "This build can't send reports yet" (`TRIAGE_KEY = None`) | Kept, with its GitHub form link; the owner's key is in `NEEDS_OWNER.md` |
| Menu "GYM OPEN" pill | Fixed text; true while the hosted runner is live | Kept |

## Release-build proof

Both builds are from `eb0da033e8`, with `CARGO_TARGET_DIR` set to a scratch
directory.

- **iOS:** `bins/openagents-ios/build.sh archive` made 1.0.0 (23), and
  `archive-source.status` was empty. The app bundle has no
  `conversation.json`.
- **Android:** `bins/openagents-android/build.sh release` made
  `OpenAgents-1.0.0-23-arm64-v8a.apk`, signed with the release key and not
  debuggable.

A byte search of the iOS binary and of the Android `libopenagents_mobile.so`
and `classes.dex` found none of these (0 hits):

- **Wallet fixture:** its Spark and deposit addresses and `spark-fixture`.
- **Chat and Gym fixtures:** their reply texts, the recorded report's digest,
  and `basic-chats-fixture`.
- **Computers fixture switch:** `OPENAGENTS_COMPUTERS_FIXTURE`.
- **Host fixture switches:** the `--rust-native-fixture`, `--gym-fixture`,
  `--chat-fixture`, `--wallet-fixture`, `--chat-script`, `--xp-preview`, and
  `--gym-preview` flags.
- **Removed screens and copy:** `NativeFixtureScreen`, `ComingSoonScreen`,
  "Coming soon.", and the "aren't open" lines.

Build 23's new strings are present, as a control. The only fixture-looking
bytes left are:

- **Launch option names:** the `Launch` field names (`gym_fixture` and the
  like), which Rust reads and ignores in release.
- **Coder's own previews:** Coder's labeled synthetic Gym board and Computers
  fixture, from the shared `verse` and `coder-computers` crates. They are
  reachable only through Coder's own C and JNI exports, which the OpenAgents
  hosts never call (they call only `openagents_*` and
  `Java_com_openagents_app_*`). Their only OpenAgents switch, `gym_preview`,
  is forced off in release.

**Android emulator:** we installed the release APK (versionCode 23, not
debuggable) and launched it with

`--ez gym_fixture true --ez chat_fixture true --ez wallet_fixture true
--ez computers_fixture true --ez gym_preview true --ez xp_preview true
--es tab wallet --es chat_script "Who are you?" --es gym_first_run done`

It ignored all of them. It opened the live first run on STEP 1 OF 3, not the
Wallet tab or the finished menu, and played no script
(`android-release-ignores-fixture-extras.png`).

## Tests

- `cargo test` for `openagents-mobile` (190 passed), for `coder-mobile`, and
  for `rust-native --all-features`.
- The `coder` Gym news tests. The worker compiles in the changelog, so these
  check build 23's words.
- `clippy -D warnings`, debug and release.

`cargo test -p rust-native -p coder-mobile` run together fails
`line_breaks_match_coretext_for_the_bundled_fonts` on `main` before this
change. It fails only under that feature unification: the corpus digest
changes. It passes per crate.

## Build 23

We archived it from `eb0da033e8` (clean; the archive says 1.0.0 (23)) and
uploaded it with `build.sh upload` at 12:41 UTC. App Store Connect (filtered
by pre-release version 1.0.0 and build 23) shows it uploaded at
2026-09-29T12:42:36Z, processing state `VALID`, internal state
`IN_BETA_TESTING`.
