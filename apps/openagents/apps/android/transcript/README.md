# Android transcript renderer

C1 (#12084) paints the shared Rust layout with Android Views. This directory
uses B1's generated Android binding and existing app setup. The C1 port is
based on B1 checkpoint `69fa3b1b5082a6e2377ad0142ae8ccb28bfad427`;
B1 (#12083) remains the native issue blocker for app integration.

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

The resumed port passed these focused checks on CoderOS over B1 checkpoint
`69fa3b1b5082a6e2377ad0142ae8ccb28bfad427` with the actual generated
Android binding. Each Gradle operation used at most two workers:

```sh
gradle :app:compileDebugKotlin :app:compileDebugUnitTestKotlin
gradle :app:testDebugUnitTest --tests com.openagents.app.transcript.TranscriptViewportTest
gradle :app:compileDebugAndroidTestKotlin
```

The JVM result is nine tests, zero failures, errors, or skips. The three
instrumentation methods compile but have not run. No C1 native rebuild, APK
assembly, emulator, or performance run took place. The existing B1 exclusion
for `platform/tests` remains outside this result; C1's explicit transcript
Android-test root was compiled.

- `src/test`: nine passing JUnit fixtures for history anchors, tail behavior, width and
  font changes, bounded reuse, empty frames, and UTF-16/scalar handling.
- `RecordedGeometry` and `RecordedText`: compact values captured from the
  existing local host library at `ac8712374b` with synthetic inputs. Geometry
  fixtures use the Rust test's `FixedFallback`. Empty display bodies in the
  stub are test doubles. These are not Android glyph metrics or device output.
- `src/androidTest`: three checks using B1's existing `InstrumentationTestCase`
  setup for actual fallback advances, Rust/Android wrapping, and exact Canvas
  text origins. The generated Android binding supplies `setDebugEntries` and
  `debugLineStarts`; no host-library binding is substituted. Run on API 26 and API 35 in
  isolated offline test state, using only synthetic layout entries.
- Capture and verification tools, full six-frame/129-row recordings, font and
  library hashes, and logs stay in the chat's `openagents scratch` directory.

After B1 integration, compile this source and run the stub tests before the
Android checks. On the isolated emulator, check earlier-history insertion,
streaming resize, font/display changes, horizontal overflow, and fast flings.
Collect `onPaintStats` plus Android frame timing; report row counts, cache
counts, allocation behavior, frame percentiles, and missed deadlines. The
paint callback's CPU duration alone does not establish frame pacing. C2 owns
streaming reveal animation and interactions; Q1 owns TalkBack and focus checks.
