# NIP implementation verification, September 26, 2026

The [implementation coverage report](../../2026-09-26-nip-implementation-coverage.md)
describes this change's supported roles and remaining work. Verification checks
those implementations; it does not certify complete support for every NIP or
establish the state of a production deployment.

## Tested code and command

The protocol implementation is revision
`c368e73b04909d2eda8cac8f3913abc7d2855d3e`. Revision
`3d96c75adf89fec2c81ba094ccd9dbe2bd8e5b3f` adds the separate Gym test-fixture
portability fix. The full workspace run at that revision passes default tests
and PostgreSQL acceptance but exposes an unrelated webhook test race in the
feature suite. The final code revision,
`b4f48f26b9439a9d5d564697cc860096cfe2a03d`, changes only that gateway test file.
It preserves all product code and adds deterministic ordering to two fixtures.

The recovery runs cover the changed gateway fixture and repeat the full
feature-enabled workspace suite. The receipts' dirty flags include verification
documents: the full run starts with no tracked diff and untracked receipts;
the recovery runs have tracked prose edits. Those documentation edits do not
change the Rust code identified above.

Host: macOS, Darwin 25.4.0, arm64. Toolchain: the repository's pinned Rust
1.97.1. The [environment record](environment.json) retains the overrides:

```sh
PATH=/opt/homebrew/opt/python@3.13/libexec/bin:$PATH \
CARGO_TARGET_DIR=/tmp/openagents-nip-sync-target \
CARGO_PROFILE_DEV_DEBUG=0 \
CARGO_PROFILE_TEST_DEBUG=0 \
CARGO_INCREMENTAL=0 \
./scripts/verify-rust.sh --keep-going
```

With the same environment, the recovery commands are:

```sh
./scripts/verify-rust.sh --crates gateway \
  --phases preflight,fmt,clippy,tests --keep-going
./scripts/verify-rust.sh \
  --phases preflight,fmt,clippy-features,tests-features --keep-going
```

The symbol and incremental settings reduce build-cache disk use. Debug
assertions remain enabled. No test assertion or acceptance threshold was
relaxed. The gate uses disposable local PostgreSQL databases; no production
database or relay was changed.

## Final result

Required manual-gate check coverage is complete across the following runs.
All 290 `nostr` library tests pass. The Gym `tui` suite passes 595 tests, with
one ignored. Default and feature-enabled workspace tests pass, with the
test-only webhook repair additionally checked in the default gateway suite.

| Run | Retained result | Passing coverage used for the final code |
| --- | --- | --- |
| [Full workspace run](webhook-fixture-failure.json) at `3d96c75adf` | **Failed** on the feature webhook race | Preflight; gate tooling, artifact, delegation, and backup tests; formatting; both strict Clippy configurations; full default workspace tests (338.4 s); dependency policy; complete live PostgreSQL acceptance (37.0 s). |
| [Default gateway recovery](gateway-recovery.json) at `b4f48f26b9` | **Partial** scope; every requested phase passes | Preflight, formatting, strict gateway Clippy, and all default gateway tests (36.2 s), including the repaired fixtures. |
| [Full workspace feature recovery](feature-recovery.json) at `b4f48f26b9` | **Partial** scope; every requested phase passes | Preflight, formatting, strict feature-enabled workspace Clippy, and all feature-enabled workspace tests (343.6 s). |

The failed full run remains marked `failed`; scoped recoveries remain marked
`partial`. This record does not relabel them as one successful full-gate
invocation. It identifies the checks that cover the final code and the test-only
change between runs.

The gate's feature combination is
`kev/serve,lev/serve,gym/tui,jev/blocking,oak/mcp-http`. Its exact commands,
durations, attempts, and tested-tree identity belong to the retained receipt.
See the [verification policy](../../../verification.md) for prerequisites and
scope.

## Relay and cryptographic evidence

The PostgreSQL phase runs the actual store, HTTP/WebSocket gateway, RS
snapshot, multiprocess, restart, Coder/worker interoperability, Block lane,
import, backup/restore, release-load, and binary deployment suites. It includes:

- Author-only app state and author/recipient private artifacts across stored
  reads, ID lookup, counts, search, and live delivery; private content stays
  out of the search index. Reserved PMA and client-authored thread bounds
  refuse publication.
- Allow/ban races from separate connections, with one consistent final policy.
- Atomic own-author snapshots independent of the ordinary history cap,
  configured Host discovery, actual-body NIP-98 authentication, durable replay
  rejection after restart, replacement and deletion, membership changes,
  corruption, and count and byte ceilings.
- Retained expired coordinates cause snapshot refusal until the real expiration
  sweep removes them. Another author's snapshot remains available.
- Channel access distinguishes closed membership from private visibility and
  rechecks access after reading. Unsupported thread modes refuse explicitly.
- Push configuration refuses until its durable delivery lifecycle exists;
  unimplemented PL and CW roles are absent from capability advertisements.

The pure `nostr` tests separately cover current official client helpers,
authenticated encrypted artifacts, market negotiation, labor terms, Block
persona/federated-identity/window helpers, and x402. The 12 x402 fixtures include
the seven retained request-binding vectors, a signed upstream invoice,
cryptographic tampering, exact terms, expiry, preimages, URI boundaries, and
strict MCP JSON. No wallet call or live Lightning payment was made.

## Earlier failures and their corrections

The earlier receipts remain unchanged:

| Receipt | Result and correction |
| --- | --- |
| [Initial scoped gate](initial-scoped.json) | Failed: Clippy found an unused helper after an obsolete push test was removed, and a gateway fixture expected EOSE despite globally required authentication. Removed the unused helper and required the correct `CLOSED auth-required` response. Anonymous store visibility has its own coverage. |
| [Disk-limited full gate](disk-limited-full.json) | Failed: both workspace test phases exhausted local build-cache space. Both Clippy runs, dependency policy, and PostgreSQL acceptance passed. Removed inactive generated build caches and rebuilt this task's target with the recorded symbol/incremental settings. |
| [Gym fixture failure](gym-fixture-failure.json) | Failed: six default and eight feature Gym tests implicitly needed absolute Linux task paths. Both Clippy runs, dependency policy, and PostgreSQL acceptance passed. The separate portability commit retains exact public task metadata and rewrites only temporary fixture paths. |
| [Webhook fixture failure](webhook-fixture-failure.json) | Failed: default workspace tests, both Clippy runs, dependency policy, and PostgreSQL acceptance passed; feature tests read the webhook delivery journal before the sender recorded the successful HTTP response. The test-only correction below makes that ordering explicit. |

The [Gym fixture provenance](../../../../crates/gym/tests/fixtures/runs/tasks/README.md)
pins the public Terminal-Bench commit, Apache 2.0 license, and eight file
digests. Original recorded answers, their evidence hashes, and assertions stay
unchanged. All four recorded answer keys match without a Jev request. No
benchmark verifier or solution was added.

The webhook receiver now holds its response behind a synchronization barrier.
The test polls for the still-absent journal before releasing that response,
then waits at most three seconds for complete newline-terminated records.
The same bounded helper covers retry completion. Non-missing I/O errors and
malformed complete JSON fail; exact signatures, record counts, attempts, and
outcomes remain asserted. This changes test synchronization, not delivery
behavior or a production deadline.

Receipts retain the local phase-log paths generated by the gate. They do not
embed the full logs. The [file digest index](log-digests.json) identifies those
original local records and logs. A receipt is an attributable local run record,
not remote attestation or proof of every protocol obligation.

## Limits

Metal checks and the long-running relay soak were not requested. External
model-weight conformance, optional live inference, production configuration,
JWT/JWKS provider interoperability, push delivery, and wallet/provider payment
interoperability are not established by this gate. Tests that skip unavailable
external prerequisites cannot establish those behaviors.

In particular, ordinary workspace tests report skips for the unconfigured
`CODER_RELAY` and `VERSE_TEST_RELAY` integration targets and the missing
`lev-bridge` helper. The PostgreSQL phase separately enables its disposable
database tests and the real Coder/worker process suite. A passing workspace
test command does not turn the other skips into live integration evidence.

The new components do not complete the remaining application runtimes in the
coverage report: for example, durable agent-labor fulfillment and acceptance,
paid settlement, or the SESS/WS/WORK/AUTO/ENV/LIVE profiles. The gate result
must not be used as a blanket NIP conformance or market-launch claim.
