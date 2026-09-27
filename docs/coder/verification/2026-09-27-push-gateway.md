# Push gateway verification — September 27, 2026

This record covers the push gateway item of
[issue #9719](https://github.com/OpenAgentsInc/openagents/issues/9719): a
gateway that holds APNs and FCM credentials for the relay's
[NIP-PL](../../../nips/block/NIP-PL.md) executor, the device side that
registers a native token and publishes its lease, and the deployment
procedure. It follows the executor record,
[push leases and activity summaries](2026-09-26-push-leases-and-activity-summaries.md).
It separates the synthetic evidence that ran from the device evidence that
did not.

## Delivered behavior

- **Gateway.** [`crates/push-gateway`](../../../crates/push-gateway) builds
  the `push-gateway` binary. A private delivery listener serves
  `/v1/deliveries/apns` and `/v1/deliveries/fcm` for the relay; a separate
  registration listener serves the installation and delegation routes for
  devices. Every request is a closed JSON object with NIP-98 authorization
  bound to the method, the signed URL's path, and the body hash; each
  authorization is accepted once.
- **Registration.** A device registers its native token under its own Nostr
  key, receives an installation handle, and asks for a delivery grant for one
  configured relay key with a strictly increasing generation. The grant
  (`pg1_…`) is what the lease carries as `endpoint`. Rotation retires every
  grant of the old epoch; revocation erases the token.
- **Delivery.** The gateway checks the signer against
  `PUSH_GATEWAY_RELAY_PUBKEYS`, resolves the grant to the held token, and
  checks transport, epoch, delegation generation, and expiry. APNs gets the PL
  constant over HTTP/2 with an ES256 provider token signed by the `.p8` key;
  FCM HTTP v1 gets `{"wake":"reconnect"}` as the complete `data` member, with
  an OAuth access token from the service account. Terminal outcomes are
  recorded by `(relay key, request_id)` and replayed without a second send;
  transient outcomes are released for the relay's next attempt. Each attempt
  allows one retry after a connection failure and one credential refresh.
  Invalid tokens answer `410 invalid_endpoint`, which the relay maps to its
  invalid-endpoint disposition.
- **Custody.** Tokens are sealed with AES-256-GCM under the operator's state
  key, bound to their installation handle; grants are stored as keyed
  digests. Credentials come only from files named by environment variables,
  and a credential file readable by group or others refuses startup. No
  token, grant, or credential appears in a log line or `Debug` output.
- **Device side.** `push_gateway::client` (feature `client`) reads the
  relay's NIP-11 `push` descriptor and `self` key, registers or rotates the
  token, delegates, builds the kind 30350 lease encrypted with NIP-44 to the
  executor key, and publishes it over NIP-42. `Enrollment` persists its state
  through the caller before it consumes a generation, renews in the last
  third of the lease lifetime, and revokes with a higher-generation
  tombstone.
- **Mobile wiring.** [`coder-mobile`](../../../crates/coder-mobile) accepts
  an optional `push` configuration (`relay_url`, `gateway_url`,
  `app_profile`) and the `push_token` and `push_disable` requests. Without the
  configuration, push is off and a request reports that. Outside test
  launches, only `wss://` relays and `https://` gateways are admitted.
  Enrollment state lives in its own encrypted cache directory.
- **Deployment.** [Push gateway](../../deployment/push-gateway.md), the
  environment template
  [`deploy/push-gateway.env.example`](../../../deploy/push-gateway.env.example)
  with placeholders only, and the hardened unit
  [`deploy/systemd/push-gateway.service`](../../../deploy/systemd/push-gateway.service).

## Synthetic evidence

All keys were generated during the run: P-256 and 2048-bit RSA keys in
temporary directories, and fixed test Nostr keys. No Apple or Google
credential, device token, or production relay was used. Checks ran on macOS.
The Postgres suite used a disposable Postgres cluster on `127.0.0.1:55447`,
created with `initdb` in a scratch directory and removed afterwards.

| Check | Result |
| --- | --- |
| `cargo test -p push-gateway --lib` | 13 passed: closed bodies (extra, duplicate, versioned, and trailing members refuse), UUID and token shapes, NIP-98 path binding, payload hash, window, and URL shape, the relay adapter's header verifying, sealed tokens bound to their handle, keyed digests, atomic state round trip and pruning, ES256 tokens verifying, loopback-only `http://` provider URLs, APNs and FCM status mapping, the FCM body carrying exactly the constant, configuration refusals, and credential redaction. |
| `cargo test -p push-gateway --test providers` | 2 passed. APNs, through a cleartext HTTP/2 fake: one wake with the exact body, `/3/device/<token>`, topic, push type, priority, `apns-id` equal to the request ID, capped expiration, and an ES256 token that verifies; replay without a send; a released transient failure; a replayed authorization refused; one refresh after `ExpiredProviderToken`; configuration fault and permanent refusal; `410 Unregistered` to invalid endpoint, then no provider call; rotation retiring the old grant; wrong owner, stale generation, unknown relay, foreign signer, expired request, wrong route, and wrong listener refused; revocation; no token or grant in the state file. FCM, through HTTP fakes: an RS256 assertion that verifies with the service-account scope and audience, the send path and bearer token, `data` equal to the constant with no `notification`, one refresh after `401`, `429` with `Retry-After: 30` reaching the relay as a 30-second retry, and `UNREGISTERED` to invalid endpoint. |
| `push-gateway` test `relay_postgres` (live) | Passed. A real relay with its PL executor and the `ApnsGateway` adapter, a running gateway, and a fake APNs: `Enrollment::sync` registers, delegates, and publishes over NIP-42; a second sync is `Current`; a mention wakes the device once with the fixed body over HTTP/2; a rotated token receives the next wake at generation 2; after `Enrollment::revoke`, no wake arrives. |
| `cargo test -p coder-mobile --lib` | 40 passed, including push off by default, the packet omitting `push` when unconfigured, and cleartext URLs refused outside test launches. `computers_live_tests::the_app_enrolls_watches_invites_sees_activity_and_is_revoked_against_a_real_host` failed in 2 of 5 full runs and passed alone and in the other 3; it does not touch push, and the failure (`host access artifact or transport check failed` while a second device redeems) is recorded here as an unrelated timing issue. |
| `cargo check -p coder-mobile --lib --target aarch64-apple-ios` | Passed. |
| `cargo check -p push-gateway --no-default-features --features client` and `--features server` | Passed. |
| `cargo clippy -p push-gateway -p coder-mobile --all-targets -- -D warnings` | Passed. |
| `cargo fmt -p push-gateway -p coder-mobile --check` | Passed. |

`scripts/test-postgres.sh` now runs `relay_postgres` after `push_postgres`.
The Android target check did not run: this machine has no Android NDK
compiler on the path for `ring`'s C code. The full workspace gate was not
run.

## Pending owner evidence

These steps need owner credentials, a deployment, and physical devices.
They have not run, and nothing here claims them.

- [ ] **APNs key.** In the Apple Developer account, open **Certificates,
  Identifiers & Profiles** > **Keys**, create a key with **Apple Push
  Notifications service (APNs)** enabled, and download the `.p8` file once.
  Install it on the gateway host as `/etc/push-gateway/apns-auth-key.p8`
  (owner `push-gateway`, mode `0400`). Set `PUSH_GATEWAY_APNS_KEY_ID` to the
  key's key ID, `PUSH_GATEWAY_APNS_TEAM_ID` to the team ID shown under
  **Membership details**, and `PUSH_GATEWAY_APNS_TOPIC` to the Coder iOS
  bundle ID.
- [ ] **iOS capability.** Under **Identifiers**, enable **Push
  Notifications** for the Coder app ID, regenerate the distribution profile,
  and add the `aps-environment` entitlement to `bins/coder-ios/host`.
  TestFlight builds use `PUSH_GATEWAY_APNS_ENVIRONMENT=production`.
- [ ] **FCM service account.** In the Firebase console, open **Project
  settings** > **Service accounts** and select **Generate new private key**.
  Install the JSON as `/etc/push-gateway/fcm-service-account.json` (owner
  `push-gateway`, mode `0400`) and set `PUSH_GATEWAY_FCM_SERVICE_ACCOUNT_FILE`
  and `PUSH_GATEWAY_FCM_APP_PROFILE`. Under **Project settings** >
  **General**, register the Android app's package and download
  `google-services.json` for the Android build.
- [ ] **State key.** Generate `/etc/push-gateway/state.key` with
  `openssl rand -hex 32` (owner `push-gateway`, mode `0400`) and set
  `PUSH_GATEWAY_STATE_KEY_FILE`. Back it up apart from the state directory.
- [ ] **Deploy.** Install the gateway beside the relay per
  [Push gateway](../../deployment/push-gateway.md), set
  `PUSH_GATEWAY_RELAY_PUBKEYS` to the relay's signing public key, proxy only
  the registration routes over TLS, and set the relay's
  `NOSTR_RELAY_PUSH_SECRET`, `NOSTR_RELAY_PUSH_TRANSPORT`,
  `NOSTR_RELAY_PUSH_APP_PROFILE`, and
  `NOSTR_RELAY_PUSH_GATEWAY=http://127.0.0.1:8090`.
- [ ] **iPhone wake.** With a TestFlight build that passes its token, confirm
  that the packet reports `Wakes on`, background the app, publish a matching
  summary, and record one wake with the fixed body followed by the app
  reading the summary.
- [ ] Repeat on an Android device through FCM, from a relay executor
  configured for the Android profile.
- [ ] Disable push in the app and record that no further wake arrives;
  reinstall or rotate the token and record that the new token receives wakes
  and the old one does not.

## What remains

- **Native token plumbing.** Neither shell requests a platform token yet. On
  iOS, `bins/coder-ios/host` needs the `aps-environment` entitlement, a
  `UNUserNotificationCenter` authorization request, and
  `registerForRemoteNotifications`, passing the device token as lowercase
  hexadecimal to `push_token`, plus the `push` object in the create
  configuration. On Android, `bins/coder-android/host` needs the Firebase
  Messaging dependency with `google-services.json`, a
  `FirebaseMessagingService` whose `onNewToken` calls `push_token`, the
  `POST_NOTIFICATIONS` permission, and the `push` configuration. A received
  wake needs no parsing: the app reconnects and reads over the relay.
- **One transport per relay.** The relay executor serves one app profile, so
  one relay wakes either iOS or Android devices. The gateway serves both.
- **Single instance.** The gateway keeps one state file per process.
- **Registration trust.** Registration is authenticated by the device's Nostr
  key, not by app attestation. A key can hold only tokens no other live owner
  holds and can only cause the fixed wake.
