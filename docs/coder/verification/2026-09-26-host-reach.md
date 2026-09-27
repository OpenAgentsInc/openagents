# Host reach verification — September 26, 2026

[Issue #9706](https://github.com/OpenAgentsInc/openagents/issues/9706) adds
[NIP-REACH](../../../nips/openagents/NIP-REACH.md) and
[`crates/coder-reach`](../../../crates/coder-reach/README.md): the owner host
directory, host presence, reachability hints, the direct-channel handshake and
frame format, and placement. It is part of the
[remote access program](https://github.com/OpenAgentsInc/openagents/issues/9704).

## Evidence class

All evidence is synthetic. Tests run in one process on one macOS computer.
Socket tests bind `127.0.0.1` port 0 and connect over loopback TCP. No relay,
emulator, simulator, physical device, second computer, LAN, or tailnet was
involved.

## Checks that ran

Each command ran from the worktree with a separate target directory:

```sh
CARGO_TARGET_DIR=target-reach cargo test -p coder-reach
CARGO_TARGET_DIR=target-reach cargo clippy -p coder-reach --all-targets -- -D warnings
cargo fmt -p coder-reach --check
```

Results: 20 unit tests and 10 socket tests passed; Clippy reported no
warnings; formatting matched. No other crate's tests ran, and the workspace
gate did not run.

## Acceptance coverage

| Acceptance item | Test |
| --- | --- |
| Directory round trip | `directory::tests::directory_round_trip` |
| Host-published directory refused | `directory::tests::host_published_directory_is_refused` |
| Presence freshness and future skew | `presence::tests::freshness_uses_receipt_time_and_refuses_future_samples`, `presence::tests::book_refuses_rollback` |
| Unknown capability handling | `presence::tests::unknown_capabilities_and_required_features`, `presence::tests::compatibility_reads_the_host_advertisement` |
| Hint selection with shareable endpoints | `hints::tests::selection_with_shareable_endpoints` |
| Hint selection without shareable endpoints | `hints::tests::selection_without_shareable_endpoints_never_falls_back_to_loopback` |
| Placement edge cases | `placement::tests::scores_by_weight_cpu_idle_and_memory`, `placement::tests::skips_stale_overloaded_zero_weight_and_incompatible`, `placement::tests::edge_cases` |
| Handshake success | `handshake_succeeds_and_carries_sequenced_encrypted_data`, `data_is_not_plaintext_on_the_wire` |
| Wrong host key | `wrong_host_key_is_refused`, `impersonating_host_fails_signature_check` |
| Replayed nonce | `replayed_nonce_is_refused` |
| Revoked grant | `revoked_grant_is_refused_after_the_device_proves_its_key`, `wrong_epoch_and_unknown_grant_are_refused` |
| Stale host generation | `stale_host_generation_is_refused` |
| Oversized frame | `oversized_frame_is_refused_before_reading_its_body`, `channel::tests::frame_reader_refuses_oversized_and_short_frames` |

Additional tests cover invalid directories, conflicting revisions, presence
from the wrong signer or owner, telemetry bounds, mislabeled and unsafe hints,
stale hint sets, sealed frames bound to their kind and sequence number, and a
hello outside the clock window.

## Limits

- The handshake is tested over TCP only. The WebSocket mapping is specified
  but has no implementation or test.
- No relay publishes or delivers these records in a test. Envelope sealing and
  opening use `nostr::private_artifact`, whose relay privacy paths are tested
  elsewhere.
- The grant check is a test double. The real host-wide grant store is issue
  #9705's work, and the host service in issue #9712 wires it.
- Selection and placement are pure functions; no test proves a real LAN,
  tailnet, or public route.
- The replay memory lives in process memory, so a host restart clears it. A
  replayed hello still cannot open a channel, because the client proof signs
  the host's fresh nonce, and the restarted host's new generation refuses the
  old hello at the verdict. The replay memory only stops the host from
  answering a copied hello with a proof.
- No fuzzing, property tests, or formal model ran for the frame parser or the
  handshake state machine.
