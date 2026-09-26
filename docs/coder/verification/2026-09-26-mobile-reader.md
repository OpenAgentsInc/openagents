# Coder iOS reader verification — September 26, 2026

This delivery implements paired read-only Codex and Claude history in the
existing Coder iOS app, tracked by #9694–#9697. The [usage guide](../guides/mobile-readonly.md)
defines its scope. The release keeps bundle `com.openagents.coder`, team
`HQWSG26L43`, and marketing version `0.5.0`; build `38` follows the existing
App Store Connect build `37`.

## What was checked

All fixtures use temporary generated histories and throwaway Nostr keys.
They start no model, agent turn, or benchmark. These checks do not publish
private user transcripts.

| Layer | Evidence |
| --- | --- |
| Host history | 14 synthetic tests and strict Clippy: catalog paging, Codex archives, Claude subagents, unknown/malformed records, large records, partial writes, source replacement, cursor binding, symlink refusal, and portable full-text projection. |
| Encrypted observer | 12 unit tests and one CLI test: NIP-42 relay exchange, signed encrypted request/reply binding, reusable connections, cancelled exchanges, expiry/revocation, rate limits, source scope, and private durable state. A separately ignored production smoke is not part of this offline count. |
| Mobile Rust application | 10 synthetic tests cover encrypted cache reopening/tamper, stale UI actions, raw record preservation, long readable messages across source pages, paused exact-page follow, catalog refresh preservation, visible eviction gaps, and rejection of an unsupported submit operation. A combined App → authenticated relay → Host fixture loads over 80 KiB, appends history, recreates the app from cache, and verifies revocation erases it. |
| Generic UI contract | 13 Rust Native tests: bounded list/text/button views, stable row IDs, schema and depth limits, Unicode/Markdown source preservation, styles, and revision-bound activation. The application palette has three separate `coder-ui` tests. |
| Existing terminal consumer | 119 `coder-terminal` tests and strict all-target Clippy check the compatibility exports after palette extraction. |
| Native boundary | Real iOS SDK typechecking, a standalone Swift decoder executable, and an arm64 simulator app linked to the actual Rust static library. No alternate Swift application state implementation. |

The native tests found a real accessibility-identifier bug: putting the ID on
a container hid its descendant controls. IDs now belong to native leaves.
The follow checks also led to a stronger contract: pausing carries the exact
page visible at gesture time, so an in-flight refresh cannot pin another page.

The production WSS check found a separate TLS-provider selection failure that
plain WebSocket fixtures could not expose. The shared transport now selects its socket-local Ring provider and WebPKI
roots explicitly. A synthetic round trip through `wss://relay.openagents.com/`
then passed: catalog plus both generated transcript records in 1.38 seconds.
No relay deployment change or private history publication was needed. This is
a functional connectivity check, not a latency benchmark or reliability claim.

## Native and distribution receipt

The committed native test target runs with `--synthetic` on a separate Keychain
identity and cache. It checks list selection, exact-source expansion, two-page
navigation, explicit resume, follow pause, and device-key persistence across
app termination/relaunch. The test target uses the real `Coder` app identity;
it does not erase other application state.

Four native UI tests passed on iPhone 17 Pro Simulator, iOS 26.5, in 49.004
seconds. The [native receipt, log, and screenshots](../../../bins/coder-ios/verification/2026-09-26/README.md)
pin the tested source files. The [observer receipt](../../../crates/coder-connect/fixtures/2026-09-26-observer/receipt.json)
retains the synthetic host/client and live-relay results.

**Coder `0.5.0 (38)` is available to the existing Internal Testers group.**
App Store Connect reports `VALID` and `IN_BETA_TESTING` for build
`03890594-261c-49a7-ba53-cc42166b00c8`. The group automatically receives all
builds; the testing notes include the pairing guide. No external beta review
or App Store production submission was requested.

The optimized archive was built from clean source commit
[`2f84cf2627`](https://github.com/OpenAgentsInc/openagents/commit/2f84cf2627777f40639411f079c26a048039f792).
Signature verification and export/upload succeeded. The
[distribution receipt](../../../bins/coder-ios/verification/2026-09-26/testflight-build38.json)
records the exact source, executable and package hashes, preserved identity,
and Apple processing state.
The release operator retains the source commit, workspace status, Cargo lock
and executable hashes, Xcode/Rust versions, `.xcresult`, archive, and export
logs outside git. No signing material or App Store Connect credentials belong
in this receipt.

## Limits and remaining work

- Physical-device Keychain locking, background suspension, VoiceOver, and the
  minimum iOS 17 runtime have not been accepted by simulator checks.
- Native Markdown preserves selectable text and exposes original Markdown;
  it is not a complete rich attachment or web renderer.
- The reader shows recorded source content. It cannot recover a missing file,
  fetch referenced attachments, or infer events the original harness did not
  persist.
- Cache, source, record, and catalog bounds are explicit in the usage guide.
  A 512 MiB source can be expensive to page because the host rechecks its prefix
  for mutation. No large-library latency or battery claim is made.
- An offline device cannot learn a new revocation until reconnecting. Expiry
  and authenticated refusal clear its local pairing and cache; already received
  content cannot be recalled from arbitrary clients.
- This is an internal TestFlight reader delivery, not full Coder suite or
  general Rust Native platform acceptance. Android, web, phone-side writing,
  task control, and paid execution remain separate work.

Only affected-package and native checks apply to this development delivery.
The repository-wide full release matrix was not run, and no unrelated issue
is held behind it.
