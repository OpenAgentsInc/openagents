# OpenAgents NIP-05 Handles

OpenAgents runs its own [NIP-05](https://github.com/nostr-protocol/nips/blob/master/05.md)
identity service so anyone can have a `name@openagents.com` Nostr handle.
The registrar lives at `apps/nip05-registrar` and serves
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
`handle -> pubkey` mapping you asked us to publish.

## Booth flow (live event)

1. Bring your `npub` to the OpenAgents booth.
2. The booth operator opens `https://openagents.com/claim` on the booth
   device.
3. Operator pastes the booth bearer token (only present on that device),
   types your handle, and pastes your `npub`.
4. The page POSTs to `https://openagents.com/admin/claim`. The registrar
   validates the handle, decodes the npub to hex, atomically rewrites
   `nostr.json`, and returns success.
5. You verify by visiting
   `https://openagents.com/.well-known/nostr.json?name=<handle>` on your
   phone. You should see your handle mapped to your hex pubkey.
6. Set `name@openagents.com` as your NIP-05 in your Nostr client.

There is no GitHub/CI/PR step for individual claims. The live host's
`nostr.json` is the source of truth during the event.

## Post-event snapshot

After the event, an OpenAgents operator may open a separate housekeeping PR
that copies the accumulated live `nostr.json` back into
`apps/nip05-registrar/data/nostr.json` so the repo state reflects the live
state. That snapshot PR is optional and is not part of any individual claim's
flow.

## Naming policy (summary)

- Handle regex: `^[a-z0-9_\-\.]{1,32}$`
- Reserved names (e.g. `admin`, `support`, `system`, …) cannot be claimed.
  Full list in `apps/nip05-registrar/policy/naming-policy.md`.
- One handle per pubkey.
- Operator can delete a wrongly-claimed handle via
  `DELETE /admin/claim/<name>` with the booth bearer token.

See `apps/nip05-registrar/policy/naming-policy.md` for the full policy.

## Operator / Chris setup

This section is for whoever runs the live registrar host.

### Assumptions

- DNS: `openagents.com` resolves to the host running the registrar.
- TLS: terminated by a fronting proxy/load balancer; the registrar binds
  HTTP on a private/internal port (default `127.0.0.1:8088`).
- Path routing: `https://openagents.com/.well-known/nostr.json` and
  `https://openagents.com/admin/claim*` proxy to the registrar.
- `https://openagents.com/claim` serves
  `apps/nip05-registrar/web/claim.html` (or proxies to it).

### Required environment variables

| Variable | Required | Default | Notes |
| --- | --- | --- | --- |
| `NIP05_REGISTRAR_ADMIN_TOKEN` | yes | — | Long random secret. Operator-only. Never commit. |
| `NIP05_REGISTRAR_LISTEN_ADDR` | no | `127.0.0.1:8088` | TCP socket the service binds. |
| `NIP05_REGISTRAR_DATA_FILE` | no | `apps/nip05-registrar/data/nostr.json` | Path to the live `nostr.json` (must be writable). |
| `NIP05_REGISTRAR_RESERVED_EXTRA` | no | — | Comma-separated extra reserved handles. |
| `RUST_LOG` | no | `warn,nip05_registrar=info` | Standard `tracing` env filter. |

Generate a token with e.g. `openssl rand -hex 32`.

### Build and run

From the repo root:

```sh
cargo build -p nip05-registrar --release
NIP05_REGISTRAR_ADMIN_TOKEN=$(openssl rand -hex 32) \
NIP05_REGISTRAR_LISTEN_ADDR=127.0.0.1:8088 \
NIP05_REGISTRAR_DATA_FILE=/var/lib/openagents/nip05/nostr.json \
./target/release/nip05-registrar
```

Front it with your existing reverse proxy (nginx, Caddy, Cloudflare Tunnel,
etc.) so `https://openagents.com/.well-known/nostr.json` and
`https://openagents.com/admin/claim*` reach the registrar. Make sure
`/.well-known/nostr.json` is reachable without auth.

### Persistence

`NIP05_REGISTRAR_DATA_FILE` should live on persistent local disk that survives
restarts. The registrar:

- Creates the file (and parent dir) on first boot if missing.
- Loads it into memory on boot.
- Atomically rewrites it on every successful claim/delete (write tmp file +
  rename).

Back it up alongside other host state. A simple cron `cp` to off-host storage
is sufficient.

### Token rotation

To rotate the bearer token:

1. Generate a new token: `openssl rand -hex 32`.
2. Update the systemd unit / process supervisor's environment.
3. Restart the registrar.
4. Update the booth device's environment (or re-paste in the claim page).
5. Old token immediately stops working.

### Operator scripts

- `apps/nip05-registrar/scripts/add_user.ps1` — non-interactive claim from a
  Windows booth device.
- `apps/nip05-registrar/scripts/verify.ps1` — fetches `nostr.json` and
  confirms a handle resolves to the expected pubkey.

### Operator responsibilities

- Hold the bearer token only on the booth device.
- Reject reserved-name claim attempts politely; offer alternates.
- Use `DELETE /admin/claim/<name>` to roll back typos immediately.
- After the event, snapshot the live `nostr.json` back to the repo via PR.
