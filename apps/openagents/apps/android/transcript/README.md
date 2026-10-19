# Android transcript renderer

C1 (#12084) paints the shared Rust layout with Android Views. This directory
uses B1's generated Android binding and existing app setup. The C1 port was
validated on B1 integration base
`fb59d69ea8798b94dc7ea69d23d637efd75be8e3` and C1 checkpoint
`2cfa7579c10577039b1478cd0e1bf02fad74efbf`.

`TranscriptCanvas` is one View, regardless of history length. It asks the
immutable Rust frame for visible rows and paints their logical coordinates
under a single density transform. It never wraps text, calculates row heights,
or rounds row origins and baselines to device pixels. Display and shaped-run
caches each hold at most 96 rows. Horizontal scroll state is also bounded.
`TranscriptViewport` preserves a row key and offset across history insertion,
streaming updates, width changes, and font scaling; dragging releases tail
following. `TranscriptFrame` allows recorded fixtures to replace the core.

The platform measurer and painter use the same APK assets, locale, font
features, and directional shaping contexts. `TranscriptFonts.faces` supplies
the exact asset bytes to Rust. Shaping a Rust-selected fragment does not
change its row, line breaks, x origin, or baseline. API 29 and later use
[`getTextRunAdvances`](https://developer.android.com/reference/android/graphics/Paint#getTextRunAdvances(char[],%20int,%20int,%20int,%20int,%20boolean,%20float[],%20int));
API 26–28 use `getRunAdvance` at ICU grapheme boundaries.
Both paths fold UTF-16 advances to one value per Unicode scalar. The API 26
path and real Android fallback behavior still need the queued device checks.

## B1 integration

1. The app source sets include the transcript main, JVM test, and Android
   test roots. Use the existing `uniffi.openagents_core` Android binding;
   C1 does not edit or generate another binding.
2. All 12 files named in `TranscriptFonts.geistAssetPaths` are bundled under
   `assets/fonts/`, with the same bytes as `crates/ui/assets/fonts`. Construct
   `TranscriptFonts` and a shared `TextSystem(fonts.faces,
   AndroidTranscriptMeasurer(fonts))` once per process, not once per row or
   chat. Rust retains these font bytes for the process.
3. Supply `TranscriptPalette` from the shell's oa-tokens palette. Construct a
   `TranscriptCanvas`. Create a `TranscriptFrameRelay` whose `latest` callback
   wraps `engine.frame()` in `RustTranscriptFrame`, then create the Rust
   `TranscriptView` with that relay. Do not attach an account in demo tests.
   `submitFrame` transfers the frame handle to the View, which closes it on
   replacement or detachment.
4. Set `onViewportChanged` to `engine.setViewport`; call `refreshViewport`
   after installing it. The scale passed to Rust is the configuration's font
   scale; style sizes returned by Rust are already scaled. Do not multiply
   them by `scaledDensity` again.
5. In `onRelease`, close the relay, call the engine's Android `shutdown()`,
   then close its handle. Detachment clears row
   caches and closes frame handles. Create a new binding after reattachment.
   Listener callbacks coalesce and read the latest frame on the main thread.
6. C2/C3 can inject `TranscriptWidgets` to draw native affordances in Rust's
   rectangles. Those callbacks run in the correct scroller coordinates. C1
   does not show nonfunctional widget controls or implement a second card.

## Check status and handoff

The integrated port passed this focused check on CoderOS over B1 checkpoint
`fb59d69ea8798b94dc7ea69d23d637efd75be8e3` with the actual generated
Android binding and at most two Gradle workers:

```sh
nix develop ./os#android --command bash -c 'cd apps/openagents && CARGO_TARGET_DIR="$HOME/work/openagents-target-1-app" scripts/android/app.sh compile :app:assembleDebug :app:assembleDebugAndroidTest'
```

The check passed in 13 seconds (86 tasks), including debug/release Kotlin,
Android-test compilation, and 15 JVM tests: nine transcript, four bootstrap,
and two push tests. XML reports show zero failures, errors, or skips. The
debug and Android-test APKs assembled with matching signing certificates. The
isolated offline API 35 AndroidJUnitRunner passed all 14 tests: two semantic
adapter, nine scanner, and three transcript text tests. The transcript tests
verified API 35 glyph advances, Rust wrapping, and Canvas text origins. API 26,
viewport runtime and performance, Activity recreation, and physical-device
checks remain unrun. Final B1 narrows its `platform/tests` exclusion to
production compilation; Android-test compilation processed both
`SemanticViewAdapterTest` and C1's explicit transcript test root.
Offline API 26/35 glyph, wrapping, runtime, and frame-pacing checks are listed
in the central master-delegation After-close table; physical qualification is
also in `NEEDS_OWNER.md`.

- `src/test`: nine passing JUnit fixtures for history anchors, tail behavior, width and
  font changes, bounded reuse, empty frames, and UTF-16/scalar handling.
- `RecordedGeometry` and `RecordedText`: compact values captured from the
  existing local host library at `ac8712374b` with synthetic inputs. Geometry
  fixtures use the Rust test's `FixedFallback`. Empty display bodies in the
  stub are test doubles. These are not Android glyph metrics or device output.
- `src/androidTest`: three checks using B1's existing `InstrumentationTestCase`
  setup for actual fallback advances, Rust/Android wrapping, and exact Canvas
  text origins. These passed on API 35 in isolated offline test state with
  synthetic layout entries. Run them on API 26 as well. The generated Android
  binding supplies `setDebugEntries` and `debugLineStarts`; no host-library
  binding is substituted.
- Capture and verification tools, full six-frame/129-row recordings, font and
  library hashes, and logs stay in the chat's `openagents scratch` directory.

After B1 integration, compile this source and run the stub tests before the
Android checks. On the isolated emulator, check earlier-history insertion,
streaming resize, font/display changes, horizontal overflow, and fast flings.
Collect `onPaintStats` plus Android frame timing; report row counts, cache
counts, allocation behavior, frame percentiles, and missed deadlines. The
paint callback's CPU duration alone does not establish frame pacing. C2 owns
streaming reveal animation and interactions; Q1 owns TalkBack and focus checks.


### Offline transcript fixture

The debug app accepts `openagents.demo=true`, `openagents.route=lab`, and
`openagents.lab=transcript`. The `transcript` route also selects this fixture.
Release ignores these extras. The fixture paints `layoutFixtureMarkdown()` through
the existing Rust `TranscriptView`, C1 Canvas, and C2 selection host. It does not
construct an account or SDK client, load media, or follow links. This supports
transcript component captures; it does not demonstrate account, wallet, saved
history, or Lab-note workflows.

The Activity closes the fixture worker, listener, selection, and frames on
background and teardown. Resume and recreation restore a row-based viewport
bookmark. The fixture has no composer or scripted streaming controls.

`DebugFixtureRoutesTest` and `AndroidDebugTranscriptFixtureTest` are authored but
unrun. The new source also awaits debug/release Kotlin and AndroidTest compilation
against the coordinator's matching Android ABI and generated bindings. No build
or runtime result is claimed for this source checkpoint.


The source-only `CoreAndroidDemo` seam can later supply the existing SDK demo's
picker, visuals, CJK, and Veil sessions to the same host/composer/transcript path.
`new(dataDir, listener)` refuses existing or relative directories and always
constructs `Credentials::Demo`; `choices()` returns typed fixture identities and
existing Rust session rows. `select(fixture)` retires the previous content;
`host()` borrows the host; `shutdown()` closes synthetic work after views close.
Calls and account routes are unavailable. Synthetic files remain for receipts.
This seam is not mounted by the debug dispatcher yet and requires matching ABI
and binding generation before Kotlin can consume it. Its Rust fixtures are unrun.
