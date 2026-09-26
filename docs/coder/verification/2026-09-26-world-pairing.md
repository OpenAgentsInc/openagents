# Verse home and QR pairing verification — September 26, 2026

[Issue #9699](https://github.com/OpenAgentsInc/openagents/issues/9699) replaces
configuration-first mobile navigation with the Verse world. Coder opens offline,
facing a computer built from shared Verse geometry. The phone opens pairing
and retained chats at that computer. The [phone guide](../guides/mobile-readonly.md)
is the current setup procedure.

## Delivered behavior

The shared world places the desk at `(0, 0, -5)`, five meters ahead of the
initial spawn. Its monitor, stand, keyboard, mouse, and legs use the existing
geometry and palette. The desk blocks movement. Rust permits interaction
within three meters, clears held input when opening, and suppresses movement
until closing. The native panel uses a projected monitor anchor and keeps the
world mounted behind it. Opening the panel does not grant access to chats. Pairing and the catalog
remain below the visible monitor; selecting a transcript expands the reading
area using Rust's `reading` state, with the world still behind it.

The computer command, `cargo run --release -p coder-connect -- connect`, shows
a QR invitation and the equivalent `coder-pair:` string, then keeps serving.
It prints the selected history folders before displaying the invitation.
Explicit folder options select only those folders; without them, it selects
existing Codex and Claude folders. The browser page is a private local file,
with no website account or inbound HTTP listener required.

The phone camera starts only after **Scan QR code**. It decodes locally,
accepts bounded invitation text, and passes it to Rust for authority checks.
Frames are not uploaded or retained. Camera denial or an unavailable camera
leaves **Paste code** usable. Backgrounding stops capture; closing the computer
releases it. The native host neither interprets transcripts nor handles pairing
authority. Rust Native remains a generic framework with no product palette,
world, camera permission, or Nostr implementation.

## Pairing authority

The [SESS bootstrap specification](../../../nips/openagents/NIP-SESS.md)
defines a five-minute invitation with a pinned host, relay, invitation ID, and
random capability. The phone signs and encrypts its request to that host.
The host atomically binds the invitation to its first accepted device and saves
the normal read-only grant before replying. Retries from that device recover
the existing grant; another device cannot reuse the invitation. Expiry,
revocation, changed roots, malformed packets, and persistence failures refuse.

The existing NIP-42 transport and encrypted private `3188` envelope carry both
bootstrap and observation. Grant and response verification stay in Rust. No
engine login, approval, execution, steering, or payment authority is transferred.
A failed scan or redemption does not replace the previous pairing or erase its
cache. Lifecycle notifications preserve a pairing failure so it remains visible.
The prior manual `pair --client` flow stays available for scripted provisioning.

## Verification

The [retained evidence](../../../bins/coder-ios/verification/2026-09-26-world-pairing/README.md)
records commands, results, images, and source digests. It separates protocol,
GPU, native UI, and distribution evidence. All chat data used in checks is
synthetic; no model, agent benchmark, or private-history publication runs here.

Checks passed: 21 connector library tests and two CLI tests; 15 mobile Rust
tests; 16 control tests covering the shared relay fixture; 105 portable and
120 desktop Verse tests; affected strict Clippy and formatting; and seven
iOS UI tests in 124.907 seconds. A synthetic bootstrap/catalog/transcript
exchange passed on the default production relay in 1.92 seconds. The shipped
Rust QR image decoded to the exact 191-byte fixture through Apple Vision
at both 520 and 1,200 pixels.

The initial native UI run exposed a container accessibility identifier that
masked child controls. Removing that container identifier preserves stable
leaf identities. That failed run remains retained. An intermediate layout left too little
space for transcript text, which the exact-source UI test exposed. Transcript
reading now expands and the test passes. Review also found
that a projected anchor could leave the viewport while its computer panel was
open; the panel now remains reachable independently of anchor visibility.

## Coverage limits

Simulator and QR-image checks do not establish a physical iPhone camera scan,
locked-device behavior, sustained battery/thermal performance, VoiceOver
navigation through the world, or all supported iOS versions. The camera uses
native AVFoundation capture, but those hardware checks remain separate.
The app is read-only. Optional world presence remains a separate Nostr identity
and explicit connection, and the desktop XP, replay picker, and world-chat
composer remain desktop features.

The static browser QR page states its lifetime and directs the operator to the
terminal for connection status. An already-open page can remain visible after
its local file is removed; the host still enforces single-device use and expiry.
The computer must stay awake and the connector must keep running for fresh
history. A relay connection cannot prove that a disconnected cache is current.

## Distribution

Coder **0.5.0 (40)** is valid and available to the internal TestFlight group.
The signed archive was built from clean commit
[`e85bfce6b4`](https://github.com/OpenAgentsInc/openagents/commit/e85bfce6b490d9b48c6a430758830bc00c316248),
which is pushed to `main`. All 103 native verification paths match the release
source, including the separately recorded test-only screenshot additions.
The [distribution receipt](../../../bins/coder-ios/verification/2026-09-26-world-pairing/testflight-build40.json)
pins the source, Cargo lock, executable, toolchain, Apple build identity, and
confirmed internal testing state. This is an internal TestFlight release;
physical-device acceptance and production App Store submission remain separate.
