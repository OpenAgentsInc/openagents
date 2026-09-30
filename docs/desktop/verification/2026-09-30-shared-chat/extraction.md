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

The extraction is an independently verified checkpoint on main. Desktop adoption
of shared projections and client state continues in #10008; the issue remains
open until both frontends use that state.
