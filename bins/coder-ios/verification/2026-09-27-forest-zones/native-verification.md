# Native zone verification

These checks use Coder 0.5.0 (47), Release Swift, optimized Rust, and the dedicated
`Coder Launch 45` iPhone 17 Pro simulator running iOS 26.5. They do not establish
physical-device acceptance.

## Shared mobile and Android checks

- `coder-mobile` library: 65 passed, one previously ignored test, no failures.
  The tests cover the actual decoded pack, session suspension and resumption,
  preserved plaza position, background cancellation, late loader results,
  pointer occlusion, cleared input, and bounded, request-only license notices.
  See [mobile-tests.log](mobile-tests.log).
- Android `scripts/build-coder-android.sh check`: Kotlin compilation,
  `lintDebug`, and JVM unit tests passed. The existing system-bar deprecation
  warnings remain. See [android-check.log](android-check.log) and
  [android-unit-tests.xml](android-unit-tests.xml). A separate Android native
  cross-build and APK packaging check also passed for `arm64-v8a`, including
  16 KiB ELF and ZIP alignment; see its [receipt](android-native-package.json)
  and [log](android-native-package.log). No new Android emulator or physical
  device zone traversal was performed.
- The final simulator binary was rebuilt after the shared zone HUD fix that
  preserves an active contact when bottom clearance is unchanged. See
  [native-release-build.log](native-release-build.log). Xcode warns that
  App Intents metadata extraction is skipped because the app does not depend
  on `AppIntents.framework`.

## Normal startup

The packaged app was installed and launched without synthetic arguments. Both
external Rust dylibs were temporarily hidden, then restored, to check that the
installed package contains its runtime. It remained alive after 10 seconds,
rendered the amber plaza, created no zone cache, and contained no `.vzp` resource.
See the [receipt](native-cold-launch.json) and
[screenshot](native-cold-launch.png).

## First native attempt

The first test run used an explicitly seeded, digest-verified device cache.
It passed both map gesture tests and reached the rendered forest through a native
portal tap. Three assertions failed:

1. The production launch check expected the old `Exploring Verse` accessibility
   value. The world now reports `Exploring Amber plaza`.
2. The zone test assumed the compact map would shrink its extent in the forest.
   Both compact maps deliberately use the same local zoom. The check now verifies
   forest landmarks and the absence of the plaza's Gym landmark.
3. SwiftUI propagated the disclosure group's accessibility identifier onto the
   nested credits text. The full notices were present in the UI hierarchy, but
   the test could not find their own identifier. The identifier now belongs to
   the disclosure label, preserving the credits text identifier.

The original failures remain in [the first log](native-first-attempt.log) and
[the first summary](native-first-attempt-summary.json). The seeded-cache receipt
is [native-cache-fixture.json](native-cache-fixture.json); this first attempt is
not evidence of HTTPS delivery.

## Final native attempt

The seeded cache was moved aside before the final run. The recorded
[HTTP starting state](native-http-start.json) identifies the absent cache and the
published, content-addressed asset URL. All five tests passed in 83.4 seconds; Xcode's total test operation was 91 seconds.
See [the final summary](native-final-summary.json) and [log](native-final.log).
The downloaded pack was independently checked at 6,629,578 bytes with SHA-256
`7c1535256a4687e70a0f624f4b97c651bfd0b36ba91347a09041698ef8d246a7`.
The [HTTP receipt](native-http-receipt.json) includes the new application
container path assigned during Xcode's test installation.

The tests exercise:

- Normal startup, background suspension, foreground resume, and cold relaunch.
- Map dragging without movement or camera input; destination walking and manual
  cancellation.
- Map navigation to the portal followed by a native screen-coordinate tap on the
  visible arch. The runtime loads the forest, shows forest landmarks, hides plaza
  interactions, starts the limited encounter, casts Fire Bolt, and returns to the
  same plaza position.
- Opening the bundled notices without loading the forest, including the Wizards
  attribution and full Apache license text.

Screenshots were visually inspected: [forest](native-forest.png),
[encounter](native-encounter.png), [return](native-return.png), and
[notices](native-notices.png). The forest renders trees, a wizard avatar, a zombie,
its own atmosphere, and the local companion; return restores amber plaza geometry.
The idle forest caption initially wrapped a word on portrait; the cosmetic
retest below verifies the correction. [The attachment manifest](native-attachments.json) maps retained images
to their exact test and timestamp.

Xcode repeatedly reports that it cannot read the LLDB debugger version. The tests
continue and pass. No application crash or renderer error was observed. This run
measures an empty-cache download and subsequent local traversal, not simultaneous
multi-user forest networking. Background loader cancellation and session
isolation are covered by the Rust tests, not a native network-fault simulation.

No archive or TestFlight upload was performed as part of this native check.


## Final caption and wrapping retest

After the five-test cold run passed, the idle caption was shortened to
`Atlantis forest · SRD 5.1`, and the GPU HUD began wrapping captions at spaces.
The optimized simulator app was rebuilt with both changes; see
[native-cosmetic-build.log](native-cosmetic-build.log).
The single portal, encounter, and return test passed again in 18.5 seconds with
the verified cache from the preceding HTTPS test. This is a separate cached
retest, not a second cold-download claim. See the [starting cache receipt](native-cached-start.json),
[test summary](native-cached-summary.json), and [log](native-cached.log).

The final [forest portrait](native-forest-final.png) and
[encounter portrait](native-encounter-final.png) were visually inspected. The
idle caption fits on one line, and the encounter caption wraps at word boundaries.
The [return screenshot](native-return-final.png) confirms the amber plaza still
renders after leaving the forest. [Attachment metadata](native-cached-attachments.json)
binds these screenshots to this final test run.
