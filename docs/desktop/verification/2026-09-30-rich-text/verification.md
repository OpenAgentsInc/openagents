# Desktop rich text verification

Issue: [#10010](https://github.com/OpenAgentsInc/openagents/issues/10010).

Rust Native now owns stable transcript selection endpoints and a bounded
paint-only syntax-span worker. This reimplements Zeron's selection and
highlighting design; no Zeron source or GPUI runtime is vendored.

## Behavior

Selection endpoints name stable row keys, paragraph fields, and UTF-8 grapheme
boundaries. Copy preserves the selected display-text bytes, including spaces,
and inserts one newline between paragraph fields and rows. Appending streamed
text, prepending history, and changing the viewport width preserve a selection.
Removing an endpoint or changing its preceding text cancels it. Copy reads the
whole retained frame, including rows outside the painted viewport. Pointer
selection clamps across paragraph gaps and margins; dragging past the viewport
scrolls it. Combining characters and emoji remain intact.

Table and code hit-testing uses the same horizontal offsets and clipping as
painting. Links change color on hover and open only after a matching press and
release without a text selection. The application admits HTTP and HTTPS links.
Code-copy preserves the original fenced block's body, including its final
newline.

The shared syntax worker uses Tree-sitter's Rust, Python, JSON, Bash,
JavaScript, TypeScript, Go, C, C++, HTML, and CSS grammars. Unknown languages
stay plain. The worker admits code up to 64 KiB, lines up to 8 KiB, and results
up to 16,384 spans. Its queue and each adapter's pending set hold at most 16
jobs; each cache retains at most 32 blocks and 2 MiB of code. Only visible code
is requested. Queue saturation leaves text readable and retries on a later
poll. The worker wakes the adapter when a result arrives. Colors apply to
already shaped glyph clusters; highlight completion never relays out a row.
Paragraph shaping caches now also have a byte bound.

The row display's `code_blocks` contract and the selection and highlighting
implementations live in shared Rust. Existing phone hosts retain their plain
code fallback until their adapter adoption in #10028.

## Checks

Rust 1.97.1, on an M5 Max Mac:

- Shared selection tests: 4 passed. The endpoint test refuses partial graphemes
  and invalidates changed text.
- Shared syntax test: 11 language fixtures preserve every input byte and
  produce multiple foreground colors; unknown and oversized inputs stay plain.
- Shared row-layout tests: 22 passed; 1 opt-in timing test ignored.
- Desktop native adapter: 47 passed. New tests copy across 50 virtualized rows,
  preserve selection through prepending and streaming, drag across margins,
  exercise scrolled link hover and matching release, and preserve code-copy.
- Shared chat application: 68 passed.
- Desktop library, binary, and pairing checks: 108 passed; 4 opt-in native/live
  benchmarks ignored in the normal suite.
- Phone Rust library with no default features: 142 passed; 19 opt-in tests ignored.
- Formatting, targeted Clippy with warnings denied, and release build passed.
- The dependency gate reports the same 29 preexisting errors as commit
  `20689a18f3`; this slice adds none. Those existing source, license, wildcard,
  and advisory failures remain outside this change. An initial Syntect trial
  introduced a Bincode maintenance advisory and was replaced before landing.

[The capture](rich-text.png) exercises headings, emphasis, inline code, links,
ordered and task lists, quotes, highlighted fenced code, copy, and tables. It
was painted through the native transcript adapter and visually inspected.
No owner host, keychain, home directory, or saved chat was used.

## Native timing

[The raw native run](native.json) uses the 3,300-row fixture at 1200 × 840 points,
2× rendering, with the Grid backdrop. Each active phase has 115 samples.

| Phase | p50 | p99 |
| --- | ---: | ---: |
| Scroll | 2.320 ms | 4.297 ms |
| Streaming | 6.582 ms | 7.599 ms |
| Sidebar | 2.217 ms | 4.111 ms |

These timings combine application work and CPU frame submission, not GPU
completion or display latency. Idle CPU was 3.31%; peak RSS was 202.8 MiB.
The earlier eight-case fixture matrix remains retained under the transcript
verification. Linux hardware timings remain an owner check in `NEEDS_OWNER.md`.
