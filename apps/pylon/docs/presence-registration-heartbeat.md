# Pylon v0.3 Presence, Registration, And Heartbeat

Status: implemented for `0.3.0-rc1` fake-server/testable client flows.

## Endpoint Authority

The Pylon CLI owns client flows for:

- `POST /api/pylons/register`
- `POST /api/pylons/:pylonRef/heartbeat`
- `POST /api/pylon-links/complete`
- `POST /api/pylon-links/refresh`

These calls publish public-safe local identity, lifecycle, capability refs,
wallet readiness state, assignment readiness state, and blocker refs. They do
not grant dispatch, spend, settlement, provider mutation, or host-code
execution authority.

## Signed Requests

Presence requests now use strict NIP-98 HTTP auth:

- `x-pylon-ref`
- `Authorization: Nostr <base64-kind-27235-event>`

The NIP-98 event is signed by the local NIP-06 Pylon key derived from
`identity.mnemonic`. The event uses kind `27235`, includes `u`, `method`, and
`payload` tags, hashes the exact JSON request body with SHA-256 hex, and signs
the event id with `secp256k1` Schnorr. The old Ed25519 `identity.json` signer
and `x-nip98-*` custom headers are no longer used for Pylon Nostr-bound
requests.

Link complete and refresh bodies also include their body hash as a field so
endpoint handlers can validate request integrity before linking a Pylon
identity.

## CLI

```sh
pylon presence register --base-url https://openagents.com
pylon presence heartbeat --base-url https://openagents.com
pylon presence link-complete --base-url https://openagents.com
pylon presence link-refresh --base-url https://openagents.com
```

`PYLON_OPENAGENTS_BASE_URL` may be used instead of `--base-url`. The dashboard
heartbeat loop only attempts live registration/heartbeat when that environment
variable is present. Without it, presence remains explicitly unregistered.

## Freshness And Blockers

Presence state persists in `presence-state.json` with:

- `registered`
- `linked`
- `stale`
- `registrationRef`
- `linkRef`
- `lastHeartbeatAt`
- `heartbeatSequence`
- `blockerRefs`

Stale presence degrades to explicit blocker refs such as
`blocker.presence.never_heartbeat` or
`blocker.presence.stale_heartbeat`. Public counters must not stay green from
old heartbeat rows.

## Redaction

Presence bodies and responses pass through the public projection guard in
`src/state.ts`. Raw secrets, auth paths, wallet material, provider credentials,
raw prompts, private repo content, private topology, and capacity-pool secrets
are rejected before projection.
