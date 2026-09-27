# Push gateway

The push gateway holds the APNs and FCM credentials for the relay's
[NIP-PL](../../nips/block/NIP-PL.md) executor. The relay never holds a
provider credential or a device token. When a lease matches, the relay posts
the closed relay-delivery request `{v, endpoint_grant, request_id,
expires_at}` with NIP-98 authorization; the gateway resolves the opaque grant
to a device token it holds and sends the platform's fixed wake constant. The
wake carries no event, relay, or lease content.

The gateway is the `push-gateway` binary from
[`crates/push-gateway`](../../crates/push-gateway). It is one process with
one state file. Run one instance per state directory.

## How it fits

```
device ──HTTPS──> TLS proxy ──> registration listener (127.0.0.1:8091)
device ──WSS───> relay (kind 30350 lease, endpoint = grant)
relay ──HTTP───> delivery listener (127.0.0.1:8090) ──HTTP/2──> APNs
                                                    └─HTTPS───> FCM HTTP v1
```

The gateway serves two listeners with separate routes:

| Listener | Default | Routes | Who calls it |
| --- | --- | --- | --- |
| Delivery | `127.0.0.1:8090` | `POST /v1/deliveries/apns`, `POST /v1/deliveries/fcm` | The relay. The NIP-98 signer must be a key in `PUSH_GATEWAY_RELAY_PUBKEYS`. |
| Registration | `127.0.0.1:8091` | `POST /v1/installations`, `/v1/installations/endpoint`, `/v1/installations/revoke`, `/v1/delegations`, `/v1/delegations/revoke` | Devices, through a TLS proxy. The NIP-98 signer is the installation owner. |

Keep the delivery listener on loopback or a private link: the relay speaks
plain HTTP to it. Publish only the registration listener.

## Registration protocol

Every registration request is a closed JSON object with `"v":1`, at most 8,192
bytes, sent as `application/json` with a NIP-98 `Authorization: Nostr …`
header signed by the device key. The gateway checks the signature, kind
`27235`, a timestamp within 60 seconds, method `POST`, the signed URL's path
(not its host), and the SHA-256 of the body. Each authorization is accepted
once.

1. **Install.** `POST /v1/installations` with `app_profile`, `endpoint` (the
   native token), and `expires_at`. The answer is `201
   {"installation_handle","endpoint_epoch","expires_at"}`. APNs tokens are
   lowercase hexadecimal; FCM tokens are printable ASCII. A token held by
   another owner is `409 installation_conflict`; the same owner registering
   the same token again gets its existing handle.
2. **Delegate.** `POST /v1/delegations` with `installation_handle`,
   `endpoint_epoch`, `generation`, `relay_pubkey`, `not_before`, and
   `expires_at`. The relay key must be configured, and `generation` must
   increase for each installation and relay. The answer is `201
   {"endpoint_grant":"pg1_…"}`. The device puts that grant in its lease as the
   encrypted `endpoint`, never the native token. A newer delegation retires
   the older grant for the same relay.
3. **Rotate.** `POST /v1/installations/endpoint` with the handle, the current
   epoch, `new_endpoint_epoch` (current plus one), and the new token. Every
   grant for the old epoch stops working.
4. **Revoke.** `POST /v1/delegations/revoke` ends one relay's grant;
   `POST /v1/installations/revoke` erases the token and every grant.

Refusals are closed bodies: `400 invalid_request`, `401 invalid_auth`,
`404 not_authorized`, `409 installation_conflict`, `429 rate_limited`, and
`503 temporarily_unavailable`. A refusal does not say whether an installation
exists.

The device side is `push_gateway::client`. `Enrollment::sync` registers or
rotates the token, delegates to the relay key that the relay's NIP-11 `self`
field names, and publishes the kind 30350 lease encrypted to the executor key
in the NIP-11 `push` descriptor. It renews in the last third of the lease
lifetime. `Enrollment::revoke` publishes a higher-generation tombstone, then
revokes the installation. The Coder mobile library exposes these as the
`push_token` and `push_disable` requests; see
[Coder mobile](../coder/guides/mobile-readonly.md).

## Delivery behavior

- **Authority.** The grant must belong to the signing relay, match the
  route's transport, the installation's current epoch, and the relay's current
  delegation generation, and satisfy `not_before <= now <= expires_at <=
  grant expiry`. Anything else is `404 invalid_grant`.
- **Fixed body.** APNs gets exactly
  `{"aps":{"alert":{"body":"Reconnect to your relay now"},"mutable-content":1}}`
  on `/3/device/<token>` over HTTP/2, with `apns-push-type: alert`,
  `apns-priority: 10`, the configured topic, `apns-id` set to the relay's
  `request_id`, and `apns-expiration` capped at one hour. FCM gets
  `{"message":{"token":…,"data":{"wake":"reconnect"},"android":{"priority":"high","ttl":"<n>s"}}}`.
- **Idempotency.** An accepted, invalid-endpoint, or permanently refused
  outcome is recorded under `(relay key, request_id)` and replayed without a
  second send. A transient outcome is released so the relay's next attempt,
  with a fresh authorization, reaches the provider again. A duplicate still in
  flight gets `503 {"status":"retry","retry_after_seconds":5}`.
- **Bounded retries.** Each attempt allows one retry after a connection
  failure and one credential refresh after an expired APNs provider token or
  a rejected FCM access token. Everything else returns to the relay, whose
  retry policy is bounded and ends in its dead-letter state.
- **Results the relay reads.** `200 {"status":"accepted"}`; `410
  {"status":"invalid_endpoint","generation":…,"invalid_at":…}` for APNs
  `410`, `BadDeviceToken`, or `DeviceTokenNotForTopic` and FCM `UNREGISTERED`,
  `SENDER_ID_MISMATCH`, or `INVALID_ARGUMENT`; `503 {"status":"retry"}` for
  provider throttling and outages, with FCM's `Retry-After`; `503
  {"error":"configuration_fault"}` for credential or topic faults; `400
  invalid_request` for a permanent provider refusal; and `429 rate_limited`
  when an installation exceeds `PUSH_GATEWAY_WAKES_PER_HOUR`. After an invalid
  endpoint, later wakes for that installation answer `410` without a send
  until the device rotates its token.

## Configuration

Credentials come only from files, read once at startup. On Unix, a credential
file that its group or other users can read refuses startup; use mode `0600`
or `0400`. No credential, token, or grant appears in a log line. The template
is [`deploy/push-gateway.env.example`](../../deploy/push-gateway.env.example).

| Variable | Required | Default | Meaning |
| --- | --- | --- | --- |
| `PUSH_GATEWAY_DELIVERY_ADDR` | no | `127.0.0.1:8090` | Delivery listener. Keep it private. |
| `PUSH_GATEWAY_REGISTRATION_ADDR` | no | `127.0.0.1:8091` | Registration listener, behind the TLS proxy. |
| `PUSH_GATEWAY_STATE_DIR` | yes | — | Directory for `state.json`. The file is replaced atomically with mode `0600`. |
| `PUSH_GATEWAY_STATE_KEY_FILE` | yes | — | File holding 64 hexadecimal characters. Tokens are sealed with AES-256-GCM under this key, and grants are stored as keyed digests. Losing the key makes every held token unreadable; devices then register again. |
| `PUSH_GATEWAY_RELAY_PUBKEYS` | yes | — | Comma-separated relay signing keys (the public key of each relay's `NOSTR_RELAY_SECRET_KEY`), 1–16. |
| `PUSH_GATEWAY_APNS_APP_PROFILE` | for APNs | — | The profile devices register under; equal to the relay's `NOSTR_RELAY_PUSH_APP_PROFILE`. Setting it requires the four APNs settings below. |
| `PUSH_GATEWAY_APNS_KEY_FILE` | for APNs | — | The `.p8` APNs authentication key. |
| `PUSH_GATEWAY_APNS_KEY_ID` | for APNs | — | That key's key ID. |
| `PUSH_GATEWAY_APNS_TEAM_ID` | for APNs | — | The Apple Developer team ID. |
| `PUSH_GATEWAY_APNS_TOPIC` | for APNs | — | The app's bundle ID. |
| `PUSH_GATEWAY_APNS_ENVIRONMENT` | no | `production` | `production` for TestFlight and App Store builds, `development` for builds signed with a development profile. |
| `PUSH_GATEWAY_FCM_APP_PROFILE` | for FCM | — | The FCM profile; must differ from the APNs profile. |
| `PUSH_GATEWAY_FCM_SERVICE_ACCOUNT_FILE` | for FCM | — | The service account key JSON with permission to send messages. |
| `PUSH_GATEWAY_FCM_PROJECT_ID` | no | the key file's `project_id` | The Firebase project ID. |
| `PUSH_GATEWAY_MAX_INSTALLATION_SECONDS` | no | `7776000` | Longest installation lifetime (90 days). |
| `PUSH_GATEWAY_MAX_GRANT_SECONDS` | no | `2678400` | Longest grant lifetime (31 days). Keep it above the relay's lease lifetime. |
| `PUSH_GATEWAY_WAKES_PER_HOUR` | no | `120` | Deliveries per installation per hour. |

`PUSH_GATEWAY_APNS_URL` and `PUSH_GATEWAY_FCM_URL` exist for local test
servers. They accept `https://` URLs, and `http://` only on a loopback host.
Leave them unset in production.

At least one of APNs and FCM must be configured. A setting for a transport
whose profile is unset refuses startup.

## Run it beside the relay

1. Build the binary with the pinned toolchain:

   ```sh
   cargo build --locked --release -p push-gateway --bin push-gateway
   ```

2. Install it at `/opt/push-gateway/current/push-gateway`, create the
   `push-gateway` system user, and install
   [`deploy/systemd/push-gateway.service`](../../deploy/systemd/push-gateway.service).
3. Create `/etc/push-gateway/` owned by `root:push-gateway` with mode `0750`.
   Put the credential files there, owned by `push-gateway` with mode `0400`:
   `state.key` (generate it with `openssl rand -hex 32`), the `.p8` key, and
   the service account JSON. Copy the environment template to
   `/etc/push-gateway/push-gateway.env` and fill in the placeholders.
4. Start the unit and read its log: it prints the two bound addresses.
5. In the relay's protected environment file, set
   `NOSTR_RELAY_PUSH_GATEWAY=http://127.0.0.1:8090`, the transport, and the
   same app profile, then restart the relay. The relay's systemd unit already
   allows loopback connections.
6. Publish the registration listener. With Caddy, add a site such as:

   ```
   push.example.com {
   	@registration path /v1/installations /v1/installations/* /v1/delegations /v1/delegations/*
   	handle @registration {
   		reverse_proxy 127.0.0.1:8091
   	}
   	respond 404
   }
   ```

   Do not proxy `/v1/deliveries/*`.

Back up `state.json` with the state key kept separately. Without the state
file, devices lose their grants and wake only after their next renewal
registers again.

## Limits

- One process owns one state file; there is no shared store for several
  instances. The file is rewritten on each change, which suits thousands of
  installations, not millions.
- Registration trusts the device key that signs it. The gateway does not
  attest the app; a key can register only tokens that no other live owner
  holds, and can only wake with the fixed constant.
- The relay executor serves one app profile and so one transport. To wake
  iOS and Android devices from one relay, the relay needs a second executor,
  which it does not support yet; the gateway already serves both.
- Replay protection keeps authorization event IDs in memory. After a restart,
  a captured delivery authorization can be replayed for up to 60 seconds, but
  a finished `request_id` still replays its recorded outcome without a send.
