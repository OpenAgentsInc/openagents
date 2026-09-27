# Push leases and activity summaries verification — September 26, 2026

[Issue #9711](https://github.com/OpenAgentsInc/openagents/issues/9711), part of
[#9704](https://github.com/OpenAgentsInc/openagents/issues/9704), lets a phone
learn that a host needs attention without keeping a socket open and without
private content passing through a push service. This record separates the
synthetic evidence that ran from the device evidence that did not.

## Delivered behavior

- **Activity summaries.** [NIP-WS](../../../nips/openagents/NIP-WS.md) now
  defines audience-bound activity summaries: a closed
  `openagents.activity-summary.v1` body with a phase, a headline of at most 160
  bytes, an attention reason, and an update time, sealed as a private `3188`
  artifact to each enrolled device. [`activity_summary.rs`](../../../crates/nostr/src/activity_summary.rs)
  encodes, bounds, redacts, verifies, seals, opens, and orders summaries.
  Failure detail becomes `Task failed` or `Session failed`.
- **Wake profile.** The same section registers the APNs body and a new FCM
  data constant, `{"wake":"reconnect"}`, and binds both to the Block PL
  relay-delivery request.
- **Executor.** The relay [executor](../../../crates/nostr-relay/src/gateway/push/mod.rs)
  and [store](../../../crates/nostr-relay/src/store/push.rs) commit leases
  transactionally with their generation watermark, enforce endpoint
  uniqueness and quota in that transaction, match from a durable cursor into
  idempotent jobs, claim each job for one worker at a time, recheck lease and
  read access before every send, and retry into a dead-letter state.
  [Migration 0011](../../../migrations/0011_push_executor.sql) adds the tables.
- **Transports.** The [APNs and FCM adapters](../../../crates/nostr-relay/src/gateway/push/transport.rs)
  post the closed four-member PL request with NIP-98 authorization to a
  credential-holding gateway. A test transport records requests.
- **Configuration.** Delivery stays off unless `NOSTR_RELAY_PUSH_SECRET` is
  set. A partial configuration, a push origin other than the relay URL, a
  missing relay signing key, an empty profile, or an ineligible kind refuses
  at startup. See [configuration](../../deployment/configuration.md).

## Synthetic evidence

All keys are throwaway test keys. No APNs or FCM credential, device token, or
production relay was used. Checks ran on macOS against a disposable
Postgres 16 cluster on `127.0.0.1:55433`, created with `initdb` in a
scratch directory and removed afterwards.

| Check | Result |
| --- | --- |
| `cargo test -p nostr --lib activity_summary` | 6 passed: private-artifact round trip and reader checks, 160-byte bound on a character boundary and body bound, redaction of paths, URLs, addresses, assignments, credentials, and long tokens, the generic failure phrase, closed and versioned verification, and sequence ordering. |
| `cargo test -p nostr --lib push_lease` | 1 passed, now including the FCM constant, subscription storage round trip, and FCM descriptors. |
| `cargo test -p nostr-relay --lib` | 33 passed, including the gateway request carrying only the four wake fields with a verifiable NIP-98 header, the FCM route, response classification, backoff and dead letter, stable job IDs, endpoint redaction in debug output, and the off-by-default configuration refusals. |
| `push_postgres` (live) | Passed: lease create, renew, stale generation, stale replacement, endpoint uniqueness, quota, expiry freeing an endpoint, revoke, replay after revoke, and ignored NIP-09 deletion; a job that survives a restart, one winner of two racing claims, reclaim after an expired claim, and a fenced stale finish; retries into `dead`; an invalid endpoint disabling one generation until rotation; revocation suppressing waiting and in-flight jobs; removal from a private group suppressing a matched wake; and one end-to-end wake through a running gateway with NIP-11 advertisement, followed by no wake after revocation. |
| `store_postgres`, `gateway_postgres`, `block_lane_postgres`, `read_state_snapshot_postgres`, `multiprocess_postgres` (live) | Passed with migration 0011 applied. |
| All `nostr` and `nostr-relay` tests without a database | 454 passed, 0 failed. |
| `cargo clippy -p nostr -p nostr-relay --all-targets -- -D warnings` | Passed. |
| `cargo fmt -p nostr -p nostr-relay --check` | Passed. |

`scripts/test-postgres.sh` now runs `push_postgres` with the other live
suites. The full workspace gate was not run.

## Pending owner evidence

These steps need owner credentials and physical devices. They have not run,
and nothing here claims them.

- [ ] Deploy a push gateway that holds APNs and FCM credentials and implements
  the PL relay-delivery route, then point `NOSTR_RELAY_PUSH_GATEWAY` at it.
- [ ] On an iPhone with the app installed from TestFlight, register a lease,
  background the app, publish a matching summary, and record that one wake
  arrives with the fixed body and that the app fetches the summary.
- [ ] Repeat on an Android device through FCM.
- [ ] Revoke the device's lease and record that no further wake arrives.
- [ ] Rotate the device token and record that the new generation receives
  wakes and the old one does not.

## Limits

- No host publishes summaries yet and no client reads them; host serving
  lands with [#9712](https://github.com/OpenAgentsInc/openagents/issues/9712),
  and device enrollment rights with
  [#9705](https://github.com/OpenAgentsInc/openagents/issues/9705).
- The push gateway is outside this repository. The relay speaks HTTP without
  TLS, so the gateway belongs on loopback or a private link.
- Priority classes are validated but not sent to the transport. One executor
  serves one application profile under one current key. Delivery checks use
  direct relay and group membership, not NIP-AA virtual membership.
- Headline redaction recognizes token classes; it cannot prove that a headline
  did not come from a prompt or engine output. Hosts must build headlines from
  their own typed state.
