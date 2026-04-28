# OpenAgents NIP-05 Handles

OpenAgents runs its own [NIP-05](https://github.com/nostr-protocol/nips/blob/master/05.md)
identity service so anyone with a Nostr key can have a `name@openagents.com`
handle. The registrar lives at `apps/nip05-registrar` and serves
`https://openagents.com/.well-known/nostr.json`.

## What you get

A NIP-05 handle is just a verifiable pointer from `name@openagents.com` to
your Nostr public key. Once claimed:

- Nostr clients that support NIP-05 will display `name@openagents.com` next to
  your posts as a verified identifier.
- Other apps that look up `openagents.com/.well-known/nostr.json?name=<handle>`
  will resolve to your pubkey.

OpenAgents does not custody your key, does not sign on your behalf, and does
not track your activity through this handle. We only publish the
`handle -> pubkey` mapping you proved control of.

## Self-serve flow (existing Nostr users)

This is the default flow for anyone who already has a Nostr key. It uses an
OTP-bound canonical message that you sign with your Nostr key to prove you
control it. The registrar verifies the Schnorr signature server-side before
publishing the mapping. **No operator is required** for this flow — the
booth lead does not have to bypass key proof, and we never trust an
unauthenticated `npub` claim.

1. Visit `https://openagents.com/claim`.
2. Enter the desired handle and your `npub` (or 64-char hex pubkey).
3. Click **Request challenge**. The page calls
   `POST /claim/challenge` and the server returns:
   - a `challenge_id`,
   - a short-lived `otp` (12 chars, ~60 bits of entropy, 10-minute TTL),
   - a `nonce`, and
   - the canonical `message` to sign.
4. Sign that canonical message as a Nostr event with **kind `27235`** using
   your existing Nostr signer (NIP-07 browser extension, mobile signer,
   or `nostr-tool`). The event content must contain the canonical message
   verbatim. The event `pubkey` and signature must match the public key
   you sent in step 2. **Do not paste your nsec into the claim page**.
5. Paste the signed event JSON into the **Submit signed event** field and
   click **Complete claim**. The page calls `POST /claim/complete`.
6. The registrar verifies the Schnorr signature, that the bound pubkey
   matches, that `created_at` is within the challenge window, and that
   the canonical message is present in the event content. On success it
   atomically writes the new mapping and returns the canonical pair.
7. Verify by visiting
   `https://openagents.com/.well-known/nostr.json?name=<handle>`. You
   should see your handle mapped to your hex pubkey.

The same `/claim/challenge` + `/claim/complete` pair works from any HTTP
client — the booth UI is just a thin wrapper. A `curl` example is included
below.

### curl example

```sh
# 1. Request a challenge.
curl -fsS -X POST https://openagents.com/claim/challenge \
  -H 'content-type: application/json' \
  -d '{"name":"alice","npub":"npub1..."}' | tee challenge.json

# 2. Sign the canonical message with your Nostr signer. Keep `kind = 27235`.
#    The signed event must look like:
#    {"id":"...","pubkey":"...","created_at":...,"kind":27235,"tags":[],
#     "content":"OpenAgents NIP-05 claim proof\n...","sig":"..."}

# 3. POST it back with the same challenge_id.
curl -fsS -X POST https://openagents.com/claim/complete \
  -H 'content-type: application/json' \
  -d '{"challenge_id":"oa-...","event":{...signed event...}}'
```

### Verifying the mapping

```sh
# Should return 200 directly with no redirects. -i prints the full headers.
curl -i https://openagents.com/.well-known/nostr.json?name=alice
```

The `.well-known/nostr.json` endpoint must return **200 directly with no
3xx redirects** — NIP-05 verifiers will not follow redirects across
origins. If a reverse proxy in front of the registrar adds a redirect,
fix the proxy. The endpoint sets `Cache-Control: public, max-age=60,
must-revalidate` so verifiers don't hammer the host while still picking
up newly-claimed handles within a minute.

## Operator override (emergency / bootstrap only)

The `POST /admin/claim` endpoint exists **only** as an operator-only
escape hatch. It requires:

- `Authorization: Bearer <NIP05_REGISTRAR_ADMIN_TOKEN>`, and
- `operator_override: true` in the JSON body.

Without `operator_override` the endpoint returns `400 challenge_invalid`
and refuses the claim — operator bearer auth is **not** sufficient on its
own. Use it only to seed officially-managed reserved handles on a fresh
deploy (e.g. binding `agent` to the OpenAgents-controlled key) or to
correct an emergency. Every override claim is logged as
`event=claim_admin_override` so audit can flag misuse.

For booth events with users who can't run a Nostr signer, walk them
through the self-serve flow on the booth device's browser instead. The
operator never needs to type, see, or paste anyone else's nsec.

## Post-event snapshot

After a live event, an OpenAgents operator may open a separate
housekeeping PR that copies the accumulated live `nostr.json` back into
`apps/nip05-registrar/data/nostr.json` so the repo state reflects the
live state. That snapshot PR is optional and is not part of any
individual claim's flow.

## Naming policy (summary)

- Handle regex: `^[a-z0-9_\-\.]{1,32}$`.
- Reserved names (e.g. `admin`, `support`, `system`, `agent`, …) cannot
  be claimed at runtime via the public flow. Full list in
  `apps/nip05-registrar/policy/naming-policy.md`.
- One handle per pubkey.
- Operator can delete a wrongly-claimed handle via
  `DELETE /admin/claim/<name>` with the booth bearer token. Deletes also
  drop the corresponding entry from the `relays` map if no other handle
  still references that pubkey.

See `apps/nip05-registrar/policy/naming-policy.md` for the full policy.

## Operator / Chris setup

This section is for whoever runs the live registrar host.

### Assumptions

- DNS: `openagents.com` resolves to the host running the registrar.
- TLS: terminated by a fronting proxy/load balancer; the registrar binds
  HTTP on a private/internal port (default `127.0.0.1:8088`).
- Path routing: the proxy forwards
  - `https://openagents.com/.well-known/nostr.json` (public, must not redirect),
  - `https://openagents.com/claim`, `/claim/challenge`, `/claim/complete`
    (public, same-origin), and
  - `https://openagents.com/admin/claim*` (operator-only)
  to the registrar.
- The registrar serves `/claim` itself with strict security headers
  (CSP, `X-Frame-Options: DENY`, `Referrer-Policy: no-referrer`,
  `X-Content-Type-Options: nosniff`, `Cache-Control: no-store`). Do not
  put another HTML page at `/claim` upstream of the registrar.

### Required environment variables

| Variable | Required | Default | Notes |
| --- | --- | --- | --- |
| `NIP05_REGISTRAR_ADMIN_TOKEN` | yes | — | Long random secret. Operator-only. Never commit. |
| `NIP05_REGISTRAR_LISTEN_ADDR` | no | `127.0.0.1:8088` | TCP socket the service binds. |
| `NIP05_REGISTRAR_DATA_FILE` | no | `apps/nip05-registrar/data/nostr.json` | Path to the live `nostr.json` (must be writable). |
| `NIP05_REGISTRAR_RESERVED_EXTRA` | no | — | Comma-separated handles to **add** to the reserved list. Cannot remove built-in reserved entries; they are a hard runtime block. |
| `RUST_LOG` | no | `warn,nip05_registrar=info` | Standard `tracing` env filter. |

Generate a token with `openssl rand -hex 32`.

### Build and run

From the repo root:

```sh
cargo build -p nip05-registrar --release
NIP05_REGISTRAR_ADMIN_TOKEN=$(openssl rand -hex 32) \
NIP05_REGISTRAR_LISTEN_ADDR=127.0.0.1:8088 \
NIP05_REGISTRAR_DATA_FILE=/var/lib/openagents/nip05/nostr.json \
./target/release/nip05-registrar
```

Front it with your existing reverse proxy (nginx, Caddy, Cloudflare
Tunnel, etc.) so the listed paths reach the registrar. Make sure
`/.well-known/nostr.json` is reachable without auth and **without any
redirects**.

### Reverse-proxy rate limiting

The registrar does **not** rate-limit itself. The proxy must do that.
Recommended baseline:

- `/admin/*` — at most 30 requests per minute per source IP, denylist on
  repeated 401s.
- `/claim/challenge` — at most 30 requests per minute per source IP. The
  challenge store is in-memory and bounded but a sustained flood can
  still saturate the upstream. The store is also not durable — see below.
- `/claim/complete` — at most 60 requests per minute per source IP.
- `/.well-known/nostr.json` — should typically be served from the proxy
  cache for 60s (see `Cache-Control` from the registrar). Burst-limit
  uncached lookups to 600 rpm per IP.

Example nginx fragment:

```nginx
limit_req_zone $binary_remote_addr zone=nip05_admin:10m rate=30r/m;
limit_req_zone $binary_remote_addr zone=nip05_claim:10m rate=60r/m;

location /admin/ { limit_req zone=nip05_admin burst=10 nodelay; ... }
location /claim/ { limit_req zone=nip05_claim burst=20 nodelay; ... }
```

### Persistence and durability

`NIP05_REGISTRAR_DATA_FILE` should live on persistent local disk. The
registrar:

- Validates the file on boot and refuses to start if any handle is
  malformed, reserved, has a non-x-only-secp256k1 pubkey, has a
  duplicate pubkey across handles, or carries a malformed relay URL.
- Loads it into memory on boot.
- Atomically rewrites it on every successful claim/delete: write to a
  unique `.<name>.tmp.<rand>` file in the same directory, `flush()` and
  `fsync()` the temp file, `rename()` over the target, then `fsync()`
  the parent directory (Unix). On Windows the parent-dir fsync is a
  no-op.

**Single-writer assumption.** The registrar process is the sole writer
of `nostr.json`. Run exactly one registrar process per data file.
Concurrent writers will race on `rename()` and corrupt the snapshot.

The challenge store is **in-memory only**. Restarting the registrar
discards outstanding challenges; users simply request a new one. Do not
shard the registrar across processes — challenges issued on one node
won't be redeemable on another.

Back the data file up alongside other host state. A simple cron `cp`
to off-host storage is sufficient.

### Token rotation

To rotate the bearer token:

1. Generate a new token: `openssl rand -hex 32`.
2. Update the systemd unit / process supervisor's environment.
3. Restart the registrar.
4. Old token immediately stops working.

### Operator scripts

- `apps/nip05-registrar/scripts/add_user.ps1` — Windows operator
  override script (uses `operator_override=true` against `/admin/claim`).
- `apps/nip05-registrar/scripts/verify.ps1` — fetches `nostr.json` and
  confirms a handle resolves to the expected pubkey.

### Operator responsibilities

- Hold the bearer token only on the booth/operator device.
- For users with existing Nostr keys, prefer the self-serve `/claim`
  flow. The operator does not need the user's key material.
- Use `DELETE /admin/claim/<name>` with the bearer token to roll back
  typos immediately.
- After the event, snapshot the live `nostr.json` back to the repo via
  PR.

## Audit logging

The registrar emits structured `tracing` events at `info`/`warn` for
each interesting state transition:

- `claim_challenge_issued` — challenge created (no OTP value logged).
- `claim_challenge_rejected` — pre-signing refusal (reserved, taken, …).
- `claim_complete_succeeded` — proof verified and mapping written.
- `claim_complete_rejected` — signature/binding/expiry failure.
- `claim_admin_override` — operator override used.
- `claim_admin_rejected` — `/admin/claim` called without
  `operator_override`.
- `claim_deleted` — operator deletion.

None of these events log the bearer token or any nsec. Events do log
handle and pubkey (which are public by design).

## Security headers

- `/claim` — `Content-Security-Policy: default-src 'self'; …;
  frame-ancestors 'none'; …`, `X-Frame-Options: DENY`,
  `X-Content-Type-Options: nosniff`, `Referrer-Policy: no-referrer`,
  `Cache-Control: no-store, max-age=0`.
- `/.well-known/nostr.json` — `Cache-Control: public, max-age=60,
  must-revalidate`, `X-Content-Type-Options: nosniff`,
  `Access-Control-Allow-Origin: *` (CORS-permissive so any client can
  resolve names).
- `/admin/*` — no CORS; intended to be hit only from the operator
  device or from your own ops automation.
