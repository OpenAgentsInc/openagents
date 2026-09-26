# Verse home and QR pairing evidence

Implementation and acceptance for [#9699](https://github.com/OpenAgentsInc/openagents/issues/9699).
The [assessment](../../../../docs/coder/verification/2026-09-26-world-pairing.md)
explains the feature, fixes, and limits. Earlier build 38/39 receipts remain unchanged.

| Check | Evidence |
| --- | --- |
| Shared desktop computer geometry | [Offline GPU capture](desktop.png), [120 desktop tests](verse-desktop-tests.log) |
| Portable world and interaction | [105 portable tests](verse-portable-tests.log), [strict Clippy](verse-portable-clippy.log) |
| Phone protocol, cache, paging, and interaction | [15 Rust tests](mobile-tests.log), [strict Clippy](mobile-clippy.log) |
| Bootstrap and real CLI lifecycle | [21 library and 2 CLI tests](connector-tests.log), [host Clippy](connector-clippy.log), [client-only Clippy](connector-portable-clippy.log) |
| Shared relay fixture after mailbox-filter correction | [16 control tests](control-fixture-tests.log) |
| Default production relay with generated history | [One bootstrap, catalog, and transcript test](production-relay.log) |
| Rust QR image decoded by the shipped Apple decoder | [520-pixel receipt](qr-520.json), [1200-pixel receipt](qr-1200.json), [synthetic image](synthetic-qr.png), [runner](qr-interop.swift) |
| Affected Rust formatting | [Check output](format.log) |

The production check selects `wss://relay.openagents.com` explicitly and uses
only generated temporary history and throwaway identities. It passed in 1.92
seconds. The local protocol tests also cover expiry, malformed/wrong proofs,
concurrent claims, source changes, restart, failed persistence, and revocation.
Three connector tests are opt-in; the production-bootstrap and QR-export opt-ins
were run separately, while the earlier production-observer test stayed skipped.

## Reproduce the core checks

Use the pinned toolchain and a separate `CARGO_TARGET_DIR` for your worktree.

```sh
cargo test --locked -p coder-connect
cargo test --locked -p coder-mobile --lib
cargo test --locked -p coder-control --lib
cargo test --locked -p verse --lib
cargo test --locked -p verse --no-default-features --lib
cargo clippy --locked -p coder-connect --all-targets -- -D warnings
cargo clippy --locked -p coder-mobile --all-targets -- -D warnings
cargo fmt --check -p coder-connect -p coder-mobile -p verse
scripts/build-coder-mobile.sh sim-test
```

The separate production-relay fixture requires explicit publication intent:

```sh
CODER_CONNECT_SYNTHETIC_RELAY=wss://relay.openagents.com \
  cargo test --locked -p coder-connect production_bootstrap_reads_only_generated_history -- --ignored --nocapture
```

For QR interoperability, export with `export_synthetic_qr_fixture`, rasterize
its SVG through native Quick Look, and compile the retained runner as
`main.swift` alongside `bins/coder-ios/host/App/QRDecoder.swift`. Pass the image
and expected payload paths; the runner compares exact bytes and prints only
its receipt. The pictured QR is a generated fixture whose host store is gone,
not a usable connection to an operator's computer.

No model or benchmark was run. Image decoding proves the Rust encoder and
Apple decoder agree on these bytes; it does not prove a physical camera scan.
Native UI evidence, archive identity, and TestFlight distribution are recorded
separately below.

## Native application

Seven simulator UI tests passed in 124.907 seconds. They exercise Verse-first
startup, real Metal frame presentation, walking to and opening the computer,
reader selection and exact bytes, paging/follow, Keychain identity across
relaunch, background resume, camera-unavailable fallback, and invalid paste
refusal. The [native receipt](native-receipt.json) pins source and toolchain
information; [the complete log](simulator-tests.log) records the final run.
The failed accessibility and cramped-transcript iterations remain retained.

Pairing and the catalog stay beneath the visible monitor. Transcript reading
expands the panel, preserving the world behind it and explicit back controls.
The fixture screen images contain no personal conversations or credentials.

Images: [opening world](world.png), [computer and catalog](computer.png),
[pairing and camera fallback](pairing.png), and [expanded transcript](transcript.png).
The native receipt records which images are settled independent captures and
which come from XCTest attachments.
