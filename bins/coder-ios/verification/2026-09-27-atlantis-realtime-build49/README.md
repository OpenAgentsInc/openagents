# Build 49: real-time Atlantis

Coder **0.5.0 (49)** is uploaded and available in internal TestFlight.
[App Store Connect](testflight-build49.json) confirms `VALID` and
`IN_BETA_TESTING`. The [signed archive](archive-source.json) comes from clean
source commit `07bf4c21882a251f09182249f1ded6990d73b699`; its executable and
Cargo lockfile hashes are recorded. Bundle and code-signature checks pass,
and no forest model pack is bundled into the app. Models still load on entry.

Issue [#9730](https://github.com/OpenAgentsInc/openagents/issues/9730) replaces
the invented turn-based encounter with retained Wizard Woods code. The mobile
hotbar supplies the original three ability inputs. Enemies pursue, NPCs cast,
and projectiles, mana, cooldowns, damage, statuses, and destructible geometry
advance continuously. The original controller and terrain are retained;
shared mobile input and renderer adaptations remain in Verse.

## Evidence

- [Native verification](native-verification.md): production launch, resume,
  relaunch, runtime portal entry, moving enemies, Fireball while walking,
  cooldown recovery, notices, and return.
- [Gameplay recording](native-combat-clip.mp4): visible Fireball launch, flight,
  and burst. [Frame and clip provenance](native-video-extraction.json) records
  extraction offsets and hashes.
- [Rust verification](rust-verification.json): 186 Verse tests, 68 mobile tests,
  12 adapter tests, 12 selected upstream combat tests, and seven upstream
  controller tests pass. One mobile opt-in test and one upstream combat test
  remain ignored. Focused Clippy and desktop compilation pass.
- [Android host checks](android-check.log): Kotlin lint and JVM tests pass.
- [Bundle verification](archive-bundle-verification.json): the signed device
  app has no dependency on a Rust library outside its bundle.
- [Source audit](../../../../docs/verse/atlantis-source-parity.md): exact source
  mapping, 198 verified retained files, scene data, host changes, and original
  defects that remain visible.

The first native batch passes three tests; the final affected-flow rerun passes
one. These are simulator observations, not physical-device acceptance or an
Android gameplay acceptance. No model or benchmark experiment was run. The
full workspace release gate was not a prerequisite for this targeted update.
Original vendor files and raw command logs retain their original whitespace.
