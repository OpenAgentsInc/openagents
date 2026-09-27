# Relay push profiles verification — September 27, 2026

This record covers the **One relay, both platforms** item of
[issue #9723](https://github.com/OpenAgentsInc/openagents/issues/9723). Before
this change, the relay's [NIP-PL](../../../nips/block/NIP-PL.md) executor
served one application profile, so one relay woke either iPhones or Android
phones. It follows the
[push gateway record](2026-09-27-push-gateway.md) and the
[push leases record](2026-09-26-push-leases-and-activity-summaries.md).

## Delivered behavior

- **Several profiles in one executor.**
  [`PushExecutor`](../../../crates/nostr-relay/src/gateway/push/mod.rs) holds a
  list of `PushProfile` values, each with its profile id, transport, optional
  retry policy, and optional per-author lease quota. `PushExecutor::new` still
  builds the one-profile executor, and `PushExecutor::with_profiles` builds
  several. The descriptor in
  [`push_lease`](../../../crates/nostr/src/push_lease.rs) carries every
  profile, and NIP-11 advertises each in `push.app_profiles` with a
  `class_support` entry for each transport.
- **Selection per lease.** An active lease is accepted only when its
  encrypted `app_profile` is configured with the lease's `transport`, as
  NIP-PL acceptance step 7 requires. An unknown profile, and a known profile
  named with the other transport, receive `invalid: transport mismatch`.
  Revocation checks no profile, so a withdrawn profile's lease can still be
  revoked.
- **Per-profile delivery.** Each wake job already recorded its profile and
  transport when it matched. The worker now looks up that profile and sends
  through its transport with its retry policy, falling back to the executor
  default. A job whose profile is no longer configured is suppressed as
  `profile_withdrawn` and is never sent through another transport. Job
  claims, the claim fence, the pre-send recheck of lease generation, expiry,
  endpoint, and read access, the backoff, and the dead-letter state are
  unchanged. The job ID still includes the profile and transport.
- **Per-profile quota.** A profile's `max_leases_per_pubkey` is checked in the
  same admission transaction as endpoint uniqueness and the origin-wide
  quota, and refuses with `invalid: lease quota exceeded`. It must be between
  1 and the origin-wide quota.
- **Configuration.** [`config.rs`](../../../crates/nostr-relay/src/gateway/config.rs)
  keeps the single-profile form (`NOSTR_RELAY_PUSH_APP_PROFILE`,
  `_TRANSPORT`, `_GATEWAY`) unchanged and adds a multi-profile form:
  `NOSTR_RELAY_PUSH_PROFILES=IOS,ANDROID` with
  `NOSTR_RELAY_PUSH_PROFILE_<LABEL>_{APP_PROFILE,TRANSPORT,GATEWAY}` and
  optional `MAX_LEASES`, `MAX_ATTEMPTS`, `RETRY_BASE_SECONDS`, and
  `RETRY_MAX_SECONDS`. Each profile has its own gateway URL; the relay's
  signing key stays the one relay identity that NIP-98 requires, and the
  platform credentials stay in the gateway. Delivery is still off without
  `NOSTR_RELAY_PUSH_SECRET`. Startup refuses any push setting without the
  secret, both forms at once, a listed label with a missing setting, a
  setting for an unlisted label, an unknown push setting, a bad label, a
  repeated profile id, a transport other than `apns` or `fcm`, and a profile
  quota above the origin quota. The migration path from the single-profile
  form is documented in
  [Push application profiles](../../deployment/configuration.md#push-application-profiles).
  No database migration is needed: stored leases and jobs already record
  their profile and transport.
- **Retry authorization fix.** The relay signs NIP-98 deterministically, and a
  retry of one job carries the same body. A retry signed in the same second as
  the previous attempt therefore repeated its event ID, which the gateway
  burns, and the retry failed as `invalid_grant` and moved to the dead-letter
  state. The live test below found it with a 1-second FCM backoff (1 failure
  in about 14 runs). Each delivery authorization now carries an `attempt` tag
  that is unique per attempt, so the retry reaches the provider. With the
  default 10-second backoff the defect was latent.
- **Gateway.** The gateway's contract did not change: it already served an
  APNs profile and an FCM profile, and its device client already read every
  advertised profile. One gateway test that asserted the old same-second
  refusal now asserts that the immediate retry is accepted.

## Synthetic evidence

All keys were generated during the run or are fixed test keys: a P-256 key
and a 2048-bit RSA key in temporary directories, and fixed Nostr keys. No
Apple or Google credential, device token, or production relay was used. The
Postgres suites used a disposable Postgres 16 cluster created with `initdb` in
a scratch directory, listening only on a Unix socket, and stopped and removed
afterwards. Checks ran on macOS.

| Check | Result |
| --- | --- |
| `cargo test -p nostr --lib` | 314 passed, including a descriptor with an APNs and an FCM profile, no profiles, a repeated profile id, and leases for each profile, each profile with the other transport, and an unknown profile. |
| `cargo test -p nostr-relay --lib` | 35 passed, including the multi-profile executor checks, the configuration forms and each refusal, and distinct authorizations for two attempts in one second. |
| `nostr-relay` test `push_postgres` (live) | Passed. The existing contract, plus one executor with an APNs profile (2 attempts, 10-second base) and an FCM profile (4 attempts, 5-second base, quota 1): NIP-11 lists both; an iOS lease and an Android lease are accepted; four mismatched or unknown profiles refuse; a second Android lease exceeds the FCM quota while a second iOS lease is accepted; one event wakes the iPhone through APNs and the Pixel through FCM, each claimed once; with both gateways failing, the APNs job reaches `dead` with `retries_exhausted` after 2 attempts while the FCM job retries on its own schedule and is delivered on attempt 3; after a restart without the FCM profile, the FCM job is suppressed as `profile_withdrawn`, APNs still delivers, and the Android lease can still be revoked. |
| `push-gateway` test `relay_postgres` (live) | Both tests passed, 22 consecutive runs after the retry fix. The new test runs one relay with an APNs profile and an FCM profile against one gateway with a fake APNs over cleartext HTTP/2 and fake FCM and OAuth servers. An iPhone and an Android phone enroll through `Enrollment`; one event wakes both, APNs at `/3/device/<token>` with the fixed body and FCM with `data` equal to the constant; a `503` from FCM retries under the FCM profile's policy and is then accepted (the gateway log shows both attempts with one request ID), while APNs delivers at once; a lease for an unknown profile and a lease for the FCM profile with `apns` are refused over the socket with `invalid: transport mismatch`. |
| `cargo test -p push-gateway --lib --test providers` | 13 and 2 passed. |
| `nostr-relay` tests `block_lane_postgres` and `gateway_postgres` (live) | 1 and 3 passed. |
| `nostr-relay` tests `deployment_static` and `store_static` | 6 and 7 passed. |
| `cargo clippy -p nostr -p nostr-relay -p push-gateway --all-targets -- -D warnings` | Passed. |
| `cargo fmt -p nostr -p nostr-relay -p push-gateway` | Applied; no remaining changes. |

The full workspace gate was not run.

## Limits

- No real APNs or FCM delivery has run. The owner steps in the
  [push gateway record](2026-09-27-push-gateway.md) still apply; for both
  platforms, configure the relay with the multi-profile form.
- One executor key serves every profile. Profiles do not have their own
  encryption keys, because NIP-PL advertises one key set per descriptor.
- The worker sends claimed jobs one at a time. A slow or unreachable gateway
  for one profile delays the other profile's jobs in the same pass by up to
  the 5-second transport timeout per job.
- Push kinds and the origin-wide limits are shared by all profiles.
