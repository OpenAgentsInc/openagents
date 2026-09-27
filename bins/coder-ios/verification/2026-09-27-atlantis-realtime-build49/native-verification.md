# Build 49 native verification

Issue [#9730](https://github.com/OpenAgentsInc/openagents/issues/9730) replaces the
forest's turn controls with the original real-time simulation and a shared GPU
ability bar. These checks ran on September 27, 2026, on the dedicated iPhone 17
Pro simulator `80A56DD2-8E81-4037-B560-2BEDBB61CAF2`, running iOS 26.5.
The app is Coder `0.5.0 (49)`, with optimized Rust and Release Swift code.

## Results

| Check | Result | Retained evidence |
| --- | --- | --- |
| Initial optimized simulator build | Passed; Rust compilation took 1 minute 29 seconds. | [Build log](simulator-first-build.log) |
| Production launch, background resume, and cold relaunch | Passed in 28.9 seconds without synthetic launch arguments. | [First test summary](native-first-summary.json), [test log](native-first.log), [launch image](native-production-launch.png), [relaunch image](native-production-relaunch.png) |
| Runtime forest load, autonomous actors, Fireball, walking, and return | Passed in 24.1 seconds in the first batch. | [First test summary](native-first-summary.json) |
| Shipped rules and artwork notices, without loading the forest | Passed in 14.8 seconds. | [Notices image](native-credits.png) |
| Final optimized build after camera clearance and defeated-player corrections | Passed; incremental Rust compilation took 14.6 seconds. | [Final build log](simulator-final-build.log), [bundle verification](simulator-bundle-verification.json) |
| Final affected native forest flow | Passed in 23.5 seconds. | [Final test summary](native-final-summary.json), [test log](native-final.log) |
| Android host lint and JVM unit tests | Passed in 24 seconds. | [Android log](android-check.log) |

All three tests in the first batch passed. The final one-test rerun covers the
affected forest flow after the last production changes. Production startup and
notices were not repeated because those changes did not affect their paths.
Both batches and their attachment manifests remain available.

## What the native test exercises

The test opens the normal Rust-rendered map and walks to the portal. It taps the
projected portal in the GPU surface, loads the reviewed runtime asset, and
observes autonomous zombie and wizard movement. There are no native spell
buttons, Start Encounter control, or End Turn control.

While the map route to the Grove is active, the test taps the rendered Fireball
button. It requires a new player cast request, a decrease in player mana, a
nonzero Fireball cooldown, disabled readiness, and new source projectiles. It
then verifies movement, waits for the ability to become ready again, and uses
the Plaza control to restore the original plaza position. The forest's combat
snapshot disappears after returning. Gym observation and the computer stay
inactive in the forest.

The [Fireball receipt](native-fireball-receipt.json) captures the exact observed
cooldown frame: the player cast count changes from 0 to 1, mana changes from 20
to 16 after regeneration, and Fireball has 0.7167 seconds remaining. The player
moves from X 8.43 to X 17.92 across the tap and observation. The
[cooldown screenshot](native-fireball-cooldown.png) shows the disabled ability
and its remaining time. This is map walking during a native ability tap;
simultaneously held movement and ability pointers are covered by shared Rust
tests, not an XCTest multitouch claim.

The source's projectile and impact counters include NPC activity. Its cast
counter counts valid player input requests, including requests the source can
later reject. The native test therefore uses mana and cooldown changes as
independent evidence that the player's Fireball was admitted. Aggregate
projectile or impact counts alone do not establish a player hit or kill. The
[final observation](native-combat-observation.json) retains the complete combat
projection for inspection.

## Visual inspection

The [native recording excerpt](native-combat-clip.mp4) shows ongoing combat,
movement, the Fireball tap, cooldown, and return. Unannotated frames extracted
from the same recording show the orange projectile at
[launch](native-fireball-launch.png), [flight](native-fireball-flight.png), and
the following [impact particles](native-fireball-impact.png). The initial
launch frame shows mana 15 and Fireball's 2.0-second cooldown. The
[extraction receipt](native-video-extraction.json) records the original video
digest, frame offsets, and the excerpt's scaling and encoding.

The [forest spawn](native-forest-spawn.png),
[ability bar](native-forest-hotbar.png), and
[returned plaza](native-return-plaza.png) were also inspected. The current
authoritative source scene has no tree instances; its open ground and ruins
are retained intentionally. This receipt does not claim that an invented
forest layout reproduces the source.

## Scope and limitations

The forest test uses the isolated synthetic identity and existing verified
asset cache; gameplay and GPU input use the production Rust simulation. It
does not inject combat outcomes or substitute a fixture battle. Production
launch tests use the normal application identity and preferences. Earlier
runtime HTTPS and cache checks remain in the
[forest-zone receipt](../2026-09-27-forest-zones/native-verification.md).

The final bundle check passes without an external Rust dynamic-library
dependency. The build reports the expected skipped App Intents metadata
warning because the app does not depend on `AppIntents.framework`.

These are simulator checks, not physical iPhone acceptance. The Android check
covers Kotlin compilation, lint, and JVM tests; it does not establish an Android
Rust cross-build, emulator gameplay, or physical-device behavior. TestFlight
archive, upload, and processing evidence are recorded separately by the release
owner.
