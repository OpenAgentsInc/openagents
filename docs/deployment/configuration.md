# Relay configuration contract

nostr-relay reads its configuration from environment variables only. There are
no configuration files and no command-line configuration flags. This adapts
the layered-configuration design of *Zero To Production In Rust* ch. 5 to a
minimal-dependency binary: the "layers" are whatever sets the environment
(systemd `EnvironmentFile`, a container platform, a shell), and the binary
sees one flat, typed contract.

## Rules

1. **Fail fast.** The binary validates all variables at startup, before it
   binds a socket or opens a database connection. On any missing required
   variable or unparsable value, it prints one clear error line to stderr
   and exits with a non-zero status.
2. **No secrets in argv.** Secrets pass only through the environment (or
   through files the environment points to). Command-line arguments are
   visible to other local users via `ps`.
3. **No secrets in logs.** Database credentials and relay signing keys never
   appear in a log line, error message, or panic output.
4. **Typed values.** Sizes are bytes, times are seconds, counts are
   integers. A value like `NOSTR_RELAY_MAX_FRAME_BYTES=abc` is a startup
   error, not a silent default.
5. **Safe defaults.** Every optional variable has a conservative default.
   A bare `DATABASE_URL=... nostr-relay` start is private (localhost bind) and
   rate-limited.

## Variables

### Database (required — one of the two forms)

| Variable | Required | Default | Meaning |
| --- | --- | --- | --- |
| `DATABASE_URL` | yes* | — | Postgres connection string, e.g. `postgres://nostr_relay:<YOUR_DB_PASSWORD>@127.0.0.1:5432/nostr-relay`. Unix-socket form is supported: `postgres://nostr_relay@%2Fvar%2Frun%2Fpostgresql/nostr-relay` or keyword form `host=/var/run/postgresql user=nostr-relay dbname=nostr_relay`. |
| `PGHOST`, `PGPORT`, `PGUSER`, `PGPASSWORD`, `PGDATABASE` | yes* | libpq-style defaults | Standard Postgres variables, used only when `DATABASE_URL` is not set. |

\* Provide a connection string or the standard Postgres variables. If both
forms are set, `DATABASE_URL` wins.

The current store connects through `tokio-postgres::NoTls`. Use a local
Unix socket or an operator-secured connection such as the documented
Cloud SQL socket. The binary does not implement direct database TLS; a managed
Postgres service that requires `sslmode=require` is unsupported without a
separately implemented and verified TLS adapter. There is no current AGENTS.md
exception or absent DigitalOcean runbook that changes this behavior.

| Variable | Required | Default | Meaning |
| --- | --- | --- | --- |
| `NOSTR_RELAY_DB_CONNECTIONS` | no | `4` | Worker database connections (1–64). Two additional dedicated connections are used for `LISTEN/NOTIFY` and the expiration sweep. |

### Network

| Variable | Required | Default | Meaning |
| --- | --- | --- | --- |
| `NOSTR_RELAY_BIND_ADDR` | no | `127.0.0.1` | Listen address. Keep the default behind a same-host reverse proxy. Set `0.0.0.0` in containers. |
| `NOSTR_RELAY_PORT` | no | `8080` | Listen port for WebSocket and NIP-11 HTTP (one port, one listener). |
| `PORT` | no | — | Platform-injected port (for example Cloud Run). When set, it overrides `NOSTR_RELAY_PORT`. |
| `NOSTR_RELAY_URL` | for NIP-42 | — | Public URL of this relay, e.g. `wss://relay.example.com`. Used to validate the `relay` tag in NIP-42 AUTH events and to advertise NIP-42 support in NIP-11. |
| `NOSTR_RELAY_AUTH_REQUIRED` | no | `false` | Require a valid per-connection NIP-42 authentication event before EVENT or REQ. `NOSTR_RELAY_URL` must be set when this is true. |

### Protocol expansion

| Variable | Required | Default | Meaning |
| --- | --- | --- | --- |
| `NOSTR_RELAY_EXPIRATION_SWEEP_SECONDS` | no | `60` | Interval for physical NIP-40 cleanup (1–86,400). Queries exclude expired events independently of the sweep. |
| `NOSTR_RELAY_SECRET_KEY` | for NIP-29 | — | Relay's 32-byte secret as 64 lowercase hexadecimal characters. Enables relay-managed groups and signed group history/metadata. The derived public key becomes the NIP-11 relay pubkey; if `NOSTR_RELAY_PUBKEY` is also set, it must match. This is a relay key, never a participant or wallet key, and belongs only in the protected runtime environment. |
| `NOSTR_RELAY_MANAGEMENT_PUBKEY` | for NIP-86 | — | Exact 32-byte owner public key as 64 lowercase hexadecimal characters. Enables the NIP-98-authenticated management endpoint. `NOSTR_RELAY_URL` is required so HTTP authorization can bind the public URL. |
| `NOSTR_RELAY_OPENAGENTS_PROFILES` | no | `false` | Advertise the implemented CAP/PRG/EXT/RUN relay roles in NIP-11. This does not run programs, decrypt private artifacts, or enable a task owner; see [retention](../protocol/openagents-retention.md). |
| `NOSTR_RELAY_READ_STATE_COMMUNITY` | for the optional NIP-RS snapshot | — | Canonical lowercase UUID. With `NOSTR_RELAY_URL`, enables the writer-database snapshot for the exact configured HTTP Host. Discovery advertises the community and snapshot limits only for that Host. No relay signing key is required. |
| `NOSTR_RELAY_PUSH_SECRET` | to enable NIP-PL | — | Executor encryption secret as 64 lowercase hexadecimal characters. Leases are encrypted to its public key. Setting it turns delivery on and makes every setting below required, except the optional ones; without it, delivery is off and any other push variable refuses startup. Requires `NOSTR_RELAY_URL`, which becomes the push origin, and `NOSTR_RELAY_SECRET_KEY`, which signs gateway requests. Protected runtime environment only. |
| `NOSTR_RELAY_PUSH_TRANSPORT` | single-profile form | — | `apns` or `fcm`. |
| `NOSTR_RELAY_PUSH_APP_PROFILE` | single-profile form | — | The application profile this executor serves, 1–512 bytes. |
| `NOSTR_RELAY_PUSH_GATEWAY` | single-profile form | — | `http://` base URL of the push gateway that holds the platform credentials, with no query or fragment. The relay posts to `/v1/deliveries/apns` or `/v1/deliveries/fcm` beneath it. The relay does not speak TLS, so place the gateway on loopback or a private link. The in-repository gateway is `push-gateway`; see [Push gateway](push-gateway.md). |
| `NOSTR_RELAY_PUSH_PROFILES` | multi-profile form | — | Comma-separated labels, 1–16, each 1–32 uppercase letters and digits, such as `IOS,ANDROID`. Each label names one application profile configured by the `NOSTR_RELAY_PUSH_PROFILE_<LABEL>_*` settings below. Setting it together with any single-profile setting refuses startup. |
| `NOSTR_RELAY_PUSH_PROFILE_<LABEL>_APP_PROFILE` | for each label | — | The profile id that leases name in `app_profile`, 1–512 bytes. Ids must be unique across labels. |
| `NOSTR_RELAY_PUSH_PROFILE_<LABEL>_TRANSPORT` | for each label | — | `apns` or `fcm`. Several profiles may share a transport. |
| `NOSTR_RELAY_PUSH_PROFILE_<LABEL>_GATEWAY` | for each label | — | The gateway base URL for this profile, with the same rules as `NOSTR_RELAY_PUSH_GATEWAY`. Profiles may share one gateway or use separate ones. |
| `NOSTR_RELAY_PUSH_PROFILE_<LABEL>_MAX_LEASES` | no | none | Active leases per author for this profile, from 1 to the origin-wide 16. The origin-wide quota still applies. |
| `NOSTR_RELAY_PUSH_PROFILE_<LABEL>_MAX_ATTEMPTS`, `_RETRY_BASE_SECONDS`, `_RETRY_MAX_SECONDS` | no | the executor values | This profile's retry bounds, with the same ranges as the executor-wide settings. A value left unset takes the executor-wide value. |
| `NOSTR_RELAY_PUSH_KINDS` | no | `1,7,9,1059,3188` | Comma-separated kinds a lease may name. Ephemeral, presence, and relay-signed snapshot kinds refuse. |
| `NOSTR_RELAY_PUSH_MAX_ATTEMPTS` | no | `5` | Attempts per wake, including the first (1–20), for profiles without their own value. |
| `NOSTR_RELAY_PUSH_RETRY_BASE_SECONDS`, `NOSTR_RELAY_PUSH_RETRY_MAX_SECONDS` | no | `10`, `600` | Exponential backoff start and ceiling for profiles without their own values. The ceiling is at most 86,400. |

#### Push application profiles

One relay can wake iPhones and Android phones at once. Name the profiles in
one of two forms:

- **Single-profile form.** Set `NOSTR_RELAY_PUSH_APP_PROFILE`,
  `NOSTR_RELAY_PUSH_TRANSPORT`, and `NOSTR_RELAY_PUSH_GATEWAY`. Existing
  installations keep working without a change.
- **Multi-profile form.** List labels in `NOSTR_RELAY_PUSH_PROFILES` and set
  the three required `NOSTR_RELAY_PUSH_PROFILE_<LABEL>_*` settings for each.

To move from the single-profile form to the multi-profile form, rename the
three settings under one label and keep the same profile id, for example
`NOSTR_RELAY_PUSH_PROFILES=IOS` and
`NOSTR_RELAY_PUSH_PROFILE_IOS_APP_PROFILE=<the old NOSTR_RELAY_PUSH_APP_PROFILE>`.
Stored leases and wake jobs record their profile id and transport, so they
carry over. Then add the second label.

The relay advertises every profile in NIP-11 `push.app_profiles` and accepts
an active lease only when its `app_profile` is configured with the same
`transport`; any other lease receives `invalid: transport mismatch`. Each
wake job keeps the profile chosen when it matched, and the worker sends it
through that profile's gateway with that profile's retry bounds. If you remove
a profile, its pending jobs are suppressed as `profile_withdrawn`, and its
leases can still be revoked. Startup refuses a listed label with a missing
setting, a setting for an unlisted label, an unknown push setting, a repeated
profile id, and a transport other than `apns` or `fcm`.

### Media

| Variable | Required | Default | Meaning |
| --- | --- | --- | --- |
| `NOSTR_RELAY_MEDIA_ROOT` | to enable M7 | — | Writable persistent directory for content-addressed Blossom bytes. Enabling it requires `NOSTR_RELAY_URL`. The committed Debian environment uses `/var/lib/nostr-relay/media`. |
| `NOSTR_RELAY_MEDIA_CLOUD_BASE_URL` | no | — | Enables the mounted-cloud adapter. Bytes are still atomically written through `NOSTR_RELAY_MEDIA_ROOT`; reads redirect beneath this base using the immutable storage key and SHA-256. The mount and public URL must obey `docs/protocol/media.md`. |
| `NOSTR_RELAY_MEDIA_MAX_BLOB_BYTES` | no | `10485760` | Maximum upload body, 1,024–1,073,741,824 bytes. Enforced from `Content-Length` before streaming. |
| `NOSTR_RELAY_MEDIA_MAX_BYTES_PER_PUBKEY` | no | `1073741824` | Maximum owned bytes per authenticated pubkey, at least the blob limit and at most 1 TiB. Shared blobs count toward each owner. |

Media is disabled when `NOSTR_RELAY_MEDIA_ROOT` is absent. The filesystem is the
default backend. Container deployments must bind-mount it persistently; do not
enable it on an ephemeral container filesystem. Upload and delete use one-use
NIP-98 events, while content-addressed GET and HEAD are public. The exact M7
surface and the deliberate Blossom BUD-11 authentication difference are in
`docs/protocol/media.md`.

NIP-17/NIP-70 delivery and publication checks are enabled when
`NOSTR_RELAY_URL` creates per-connection NIP-42 state. NIP-45 COUNT,
NIP-50 search, and NIP-65 relay-list storage need no extra variable. The full
contract and deliberate NIP-29 subset are in
`docs/protocol/nip-expansion.md`.

The Block extension handlers need no additional service or database. NIP-AO
uses the dedicated observer rates below. NIP-IA, NIP-DV, and NIP-CW require
`NOSTR_RELAY_SECRET_KEY` because their derived state is relay-signed. NIP-CW
also requires `NOSTR_RELAY_URL` and serves `POST /query`. NIP-WP requires
`NOSTR_RELAY_MANAGEMENT_PUBKEY`. The optional NIP-RS snapshot requires
`NOSTR_RELAY_READ_STATE_COMMUNITY` and `NOSTR_RELAY_URL`; it accepts only
an authenticated own-author kind-30078 snapshot request. Configure the proxy
to preserve the configured Host. Forwarded headers do not select a community.
The endpoint refuses incomplete or oversized snapshots instead of paginating
or returning an ordinary event array. NIP-PL delivery is off unless
`NOSTR_RELAY_PUSH_SECRET` is set. See `docs/protocol/block-nips.md`.

TLS terminates at the reverse proxy. The binary itself never speaks TLS and
has no certificate configuration.

### Limits (all enforced; fail closed)

| Variable | Required | Default | Meaning |
| --- | --- | --- | --- |
| `NOSTR_RELAY_MAX_FRAME_BYTES` | no | `131072` | Maximum WebSocket frame/message and gateway event size in bytes (1,024–16,777,216). Larger frames close the connection; larger publications are refused. The database admission policy has its own content bound. |
| `NOSTR_RELAY_MAX_SUBSCRIPTIONS` | no | `32` | Maximum concurrent subscriptions per connection (1–1,024). Excess `REQ` is answered with `CLOSED`. |
| `NOSTR_RELAY_MAX_FILTERS` | no | `16` | Maximum filters per `REQ` (1–256). |
| `NOSTR_RELAY_MAX_LIMIT` | no | `1000` | Cap on any filter `limit` (1–100,000); also the default page size when a filter has no `limit`. One `REQ` returns at most the smaller of this and `(NOSTR_RELAY_SEND_QUEUE_CAPACITY - 1) / 2` stored events, split across its filters, so the defaults cap a `REQ` at **127** events. NIP-11 advertises that effective cap as `limitation.max_limit` and `default_limit`; a truncated answer ends with a NIP-67 `["EOSE", id, ["more"]]`, and a client pages with `until` to get the rest. |
| `NOSTR_RELAY_MAX_QUERY_COST` | no | `100000` | Upper bound on estimated rows scanned per `REQ` (1–1,000,000,000); costlier queries are refused with `CLOSED`. |
| `NOSTR_RELAY_RATE_EVENTS_PER_MIN_IP` | no | `120` | `EVENT` messages accepted per minute per client IP. |
| `NOSTR_RELAY_RATE_EVENTS_PER_MIN_PUBKEY` | no | `60` | `EVENT` messages accepted per minute per author pubkey. |
| `NOSTR_RELAY_RATE_GIFT_WRAPS_PER_MIN_RECIPIENT` | no | `60` | Kind-1059 gift wraps accepted per minute for each outer `p` recipient. This complements the generic IP and outer wrapper-pubkey limits; the relay cannot observe the encrypted logical sender. |
| `NOSTR_RELAY_RATE_OBSERVER_PER_SEC_IP` | no | `200` | NIP-AO observer frames accepted per second per client IP. |
| `NOSTR_RELAY_RATE_OBSERVER_PER_SEC_AGENT` | no | `100` | NIP-AO observer frames accepted per second for each agent, including owner-to-agent control traffic. |
| `NOSTR_RELAY_RATE_REQ_PER_MIN_IP` | no | `120` | `REQ` messages per minute per client IP. |
| `NOSTR_RELAY_RATE_MEDIA_PER_MIN_IP` | no | `30` | Combined media uploads and deletes per minute per client IP. |
| `NOSTR_RELAY_RATE_MEDIA_PER_MIN_PUBKEY` | no | `15` | Combined media uploads and deletes per minute per authenticated pubkey. |
| `NOSTR_RELAY_MAX_CONNECTIONS_PER_IP` | no | `20` | Concurrent WebSocket connections per client IP (1–4,096). |
| `NOSTR_RELAY_SEND_QUEUE_CAPACITY` | no | `256` | Maximum queued outbound messages per connection (8–65,536). Historical result batches and per-subscription EOSE buffers are each capped below half this value so their handoff remains bounded. A slow connection that fills the queue is closed. |

When the relay runs behind a reverse proxy, the client IP is taken from the
proxy connection's `X-Forwarded-For` header **only when**
`NOSTR_RELAY_TRUST_PROXY=true` (default `false`). Never enable it when the
binary is directly reachable. The chain is read from the right: each trusted
proxy appends the address it received the connection from, so the relay uses
the entry `NOSTR_RELAY_TRUSTED_HOPS` places from the right end and ignores
everything a client put to its left. `X-Real-IP` is used only when no
`X-Forwarded-For` entry is usable.

| Variable | Required | Default | Meaning |
| --- | --- | --- | --- |
| `NOSTR_RELAY_TRUST_PROXY` | no | `false` | Trust forwarded-IP headers from the (single) upstream proxy. |
| `NOSTR_RELAY_TRUSTED_HOPS` | no | `1` | Trusted proxies in front of the relay that each append to `X-Forwarded-For`. `1` for the shipped nginx config and for Cloud Run (its front end appends the client address); `2` for an external HTTPS load balancer in front of Cloud Run. |

### Operations

| Variable | Required | Default | Meaning |
| --- | --- | --- | --- |
| `NOSTR_RELAY_LOG_LEVEL` | no | `info` | One of `error`, `warn`, `info`, `debug`. Logs are single-line JSON on stdout. |
| `NOSTR_RELAY_SHUTDOWN_GRACE_SECONDS` | no | `10` | On SIGTERM: stop accepting, drain in-flight admissions, close connections, exit within this bound. |

## NIP-11 identity (optional, advertised only)

| Variable | Required | Default | Meaning |
| --- | --- | --- | --- |
| `NOSTR_RELAY_NAME` | no | `nostr-relay` | NIP-11 `name`. |
| `NOSTR_RELAY_DESCRIPTION` | no | empty | NIP-11 `description`. |
| `NOSTR_RELAY_CONTACT` | no | empty | NIP-11 `contact`. |
| `NOSTR_RELAY_PUBKEY` | no | empty | NIP-11 `pubkey` (operator's public key — never a private key). |
| `NOSTR_RELAY_SUPPORTED_NIPS` | no | active implemented list | Comma-separated decimal NIP numbers in the exact order advertised by NIP-11, for example `11,1,50`. Every value must be unique and supported under the active configuration. The override may narrow the list but cannot advertise an inactive or unimplemented capability. |

The limits in NIP-11 come from the enforced `NOSTR_RELAY_MAX_*` variables and
the M2 policy limits; they are not a separate claim-only configuration. The
supported-NIP override likewise changes discovery only. It cannot activate a
handler, add a protocol, or bypass the configuration that an implemented
conditional NIP requires.

## Example: minimal local start

```sh
DATABASE_URL="postgres://nostr_relay:<YOUR_DB_PASSWORD>@127.0.0.1:5432/nostr-relay" \
NOSTR_RELAY_LOG_LEVEL=info \
./nostr-relay
```

## Example: production environment file

`/etc/nostr-relay/nostr-relay.env`, owned `root:nostr-relay`, mode `0640` (see the
Debian runbook):

```sh
DATABASE_URL=postgres://nostr_relay:<YOUR_DB_PASSWORD>@127.0.0.1:5432/nostr-relay
NOSTR_RELAY_BIND_ADDR=127.0.0.1
NOSTR_RELAY_PORT=8080
NOSTR_RELAY_URL=wss://relay.example.com
NOSTR_RELAY_TRUST_PROXY=true
NOSTR_RELAY_EXPIRATION_SWEEP_SECONDS=60
NOSTR_RELAY_MEDIA_ROOT=/var/lib/nostr-relay/media
NOSTR_RELAY_MEDIA_MAX_BLOB_BYTES=10485760
NOSTR_RELAY_MEDIA_MAX_BYTES_PER_PUBKEY=1073741824
NOSTR_RELAY_LOG_LEVEL=info
```

To enable groups and management, add the relay secret and management public
key to the installed protected file; never add their real values to the
repository or shell history:

```sh
NOSTR_RELAY_SECRET_KEY=<64-lowercase-hex-secret>
NOSTR_RELAY_MANAGEMENT_PUBKEY=<64-lowercase-hex-public-key>
```

## Status note

The relay implements this configuration contract. M1–M7 are retained historical
milestone labels, not the current implementation boundary. Use the
[protocol coverage report](../protocol/2026-09-26-nip-implementation-coverage.md)
for later changes, optional roles, and unsupported features. Update this page
with configuration changes.
