# Shared chat application extraction

This checkpoint advances #10008 by moving the phone's portable controller into
`openagents-chat-app`: CoderTab, conversation reads and projection, retained
transcripts, chat lists, invitations, outbox, CLI offers, suggestions, Gym cards,
and wake state. The phone uses re-exports. Existing cache formats, opaque tokens,
NIP-HOST admission, router behavior, and packet serialization are unchanged.

The shared application crate uses the existing shared hosted-chat core and Coder
client. It has no mobile, wallet, Breez, or SQLite dependency. This explicitly
keeps the phone's SQLite workspace boundary outside the reusable chat state.
The wallet and playtest remain phone implementations behind typed destinations.
Public API visibility admits the phone as an external consumer; it grants no
additional remote rights.

On pinned Rust 1.97.1:

- Standalone shared application tests: 55 pass.
- Phone controller tests: 30 pass.
- Complete phone library tests: 142 pass, with 19 opt-in tests ignored. Wallet,
  packet, playtest, and chat consumers remain compatible.
- Shared application formatting and Clippy with warnings denied pass.
- The phone workspace passes its locked compilation check.

The extraction landed as an independently verified checkpoint on main. The
following adoption completes #10008; card mounting continues in #10009.

## Desktop adoption

The desktop now consumes the shared application's `Session` for conversation
selection, list observations, pending sends, retry identity, page merging,
polling, and late-response handling. Its `Panel` retains native fields, focus,
clipboard callbacks, and painting. Accepted sends return their conversation and
request IDs to the adapter; only its matching draft revision can clear.

Both desktop and phone project the core's `Turn` and `Summary` values through
one cached `Projection`. Saved messages parse only when changed; streaming uses
Rust Native's incremental Markdown parser. Both use the same completed-reply
policy and the same bounded used-suggestion digests. Failed, stopped, and
streaming replies cannot offer follow-ups. Source metadata stays separate from
answer text. Router offers and Coder target selection are in the shared phone
controller; desktop card mounting follows in #10009.

The core keeps lifecycle, router validation, titles, encrypted records, and
transport. The shared app keeps client state and projections. The phone adapter
still owns its wallet, playtest logger, native mounts, and SQLite workspace.
The desktop window receives typed snapshots over its private control worker and
holds no chat signing credentials.

Additional checks on pinned Rust 1.97.1:

- Shared app: 64 tests pass, including presentation parity, non-repeated
  follow-ups, failed-write recovery, exact retry bytes, late acknowledgments,
  late archive responses, out-of-order reads, and retained earlier pages.
- Desktop: 107 tests pass; four opt-in benchmarks and public smokes remain
  ignored in the ordinary run. Existing pairing and backdrop tests pass.
- Core: 26 tests pass; one public smoke is opt-in.
- Phone library: 142 tests pass; 19 opt-in checks remain ignored.
- Private host control: eight integration tests pass on temporary state.
- Targeted formatting and Clippy with warnings denied pass.

The offline native 3,300-row fixture ran at 1,200 × 840 points and 2× scale with
Verse visible. Across 115 measured samples per phase, scroll p99 is 2.595 ms,
streaming p99 is 8.697 ms, and sidebar p99 is 3.018 ms. Idle CPU is 2.79%; peak
resident memory is 179.1 MiB. These times include application work and CPU frame
submission, not GPU completion or scanout. Scroll meets the 8.3 ms target;
streaming remains above that 120 Hz budget and below the 60 Hz budget. The
[numeric report](native.json) retains every sample. The earlier full matrix and
Linux owner check remain in the transcript verification record.
