# Real-event development probe, 2026-10-29

This closes the local demonstration in #6943. The actual shared Coax event
reader, canonical ID serializer and public Schnorr verifier decide every result.
No signer, private key, alternate event decoder or production-admission path is
included. The folder is hidden from normal site deployment and served explicitly
as a standalone local app; see [index.md](index.md).

The captured public repository event is 681 bytes. Its object bytes are retained
exactly from the existing `coax/lib/nostr/fixtures/coax_relay.coax` relay transcript
captured at 2026-10-28T20:33Z. Its SHA-256 is
`dcf32812f99618c23fa3dd2bff0569ef590f2b15d2d44b61bfb61f3257f00e93`.
Its ID is
`813d9fb02bd78ce24200ba986b62ef727f0c032f712cff82241bd465f4bc9d21`.
The explicit test independently reconstructs the NIP-01 preimage with Python
JSON, hashes it with hashlib, and verifies the signature with the existing
independent arbitrary-precision BIP-340 reference. It does not sign anything.

The original shows VERIFIED. A changed ID or content returns NE_ID; a changed
signature returns NE_SIGNATURE; kind 99999 returns NE_SHAPE because it exceeds
the supported event profile. All four display REFUSED. Invalid selector values
also refuse. VERIFIED means ID and public signature verified, without granting
author, project, freshness or action authority.

The renderer and verifier are separate Coax compilation units because their
existing flat namespaces collide on names such as `field` and `frame`. The C
face only transfers scalar observations and input/render buffers. Its declaration
is compiled against the verifier's generated header in the explicit tests.
The shared event source remains byte-for-byte unchanged at SHA-256
`3b0c0feca25959ecd4a4138a3e74a795c65258198fcb1510c0f9e5b2d3e5419f`;
its completed check/mutation evidence remains in the #7cdf report. This display
task does not repeat or replace that full core gate.

Focused results, compiler identity, source hashes and artifact hashes are in
[evidence.json](evidence.json):

- Coax UI and imported renderer/HUD checks: 37/37; verifier closure: 180/180.
- Checked and release native builds each exercise seven verifier selectors,
  five keyboard selections, fifteen actual engine-hotbar clicks across desktop,
  phone and 2x device coordinates, and six cancelled or mismatched gestures.
- The actual built WebAssembly app, including its C face, exercises 30 frames,
  all five modes, keyboard/pointer inputs and state/vertex/uniform guard bytes.
  Its stack is the generic builder's explicit 1 MiB stack. The Node/V8 run is
  recorded separately from the native profiles.
- The visuals checker passes. Muted headless Chrome WebGPU renders were
  inspected for the original VERIFIED desktop screen, ID/content REFUSED desktop
  screens, and signature/profile REFUSED phone screens. The hotbar uses
  `verse/hud.coax` geometry rather than a custom layout definition.

Python is explicit test tooling only; the normal page build remains the Coax
compiler plus the C/Wasm toolchain. No physical iOS, Safari/Firefox, live relay
completeness, signing, secret storage or production authorization is claimed.
