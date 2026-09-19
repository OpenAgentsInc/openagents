# Local relay development

This runbook starts a loopback `nostr-relay` with disposable Postgres state and
drives the NIP-42 authentication handshake through it. It supports macOS and
Linux with `initdb`, `pg_ctl`, `createdb`, `cargo`, `curl`, and `nak`
installed. Nothing here touches a production database or key.

## Start disposable Postgres

```bash
pgdir="$(mktemp -d /tmp/nostr-relay-dev.XXXXXX)"
mkdir -p "$pgdir/socket"
initdb -D "$pgdir/data" -A trust --no-locale -E UTF8
pg_ctl -D "$pgdir/data" \
  -o "-c listen_addresses='127.0.0.1' -c port=55432 \
      -c unix_socket_directories='$pgdir/socket'" -w start
createdb -h "$pgdir/socket" -p 55432 -U "$(id -un)" nostr_relay_dev
```

## Build and start the relay

```bash
cargo build -p nostr-relay --bin nostr-relay

DATABASE_URL="host=127.0.0.1 port=55432 user=$(id -un) dbname=nostr_relay_dev" \
NOSTR_RELAY_PORT=8080 \
NOSTR_RELAY_URL="ws://127.0.0.1:8080" \
NOSTR_RELAY_SECRET_KEY='<64-lower-hex-development-secret>' \
NOSTR_RELAY_AUTH_REQUIRED=true \
  target/debug/nostr-relay
```

`NOSTR_RELAY_SECRET_KEY` is the relay's own signer identity: it derives the
NIP-11 `pubkey` and signs relay-managed state. Use a development key, never a
participant or wallet key. `NOSTR_RELAY_AUTH_REQUIRED=true` requires every
connection to complete NIP-42 before `EVENT` or `REQ`; it needs
`NOSTR_RELAY_URL` so the auth `relay` tag can be validated.

The relay log prints `nostr-relay listening` with the bound address.
Migrations apply automatically on first boot; the startup fails closed if the
hash ledger disagrees with the database.

## Check NIP-11 and health

```bash
curl -fsS http://127.0.0.1:8080/health
curl -fsS -H 'Accept: application/nostr+json' http://127.0.0.1:8080/
```

The NIP-11 document reports the relay name, the derived pubkey, and the
supported NIP list.

## Exercise the NIP-42 handshake

An unauthenticated request is refused:

```bash
nak req -k 1 -l 1 --sec "$NOSTR_SECRET_KEY" ws://127.0.0.1:8080
# CLOSED: auth-required: authenticate before subscribing
```

`--force-pre-auth` waits for the relay's `AUTH` challenge, signs the kind-22242
response, and only then sends the subscription:

```bash
nak req -k 1 -l 1 --force-pre-auth --sec "$NOSTR_SECRET_KEY" ws://127.0.0.1:8080
```

Publish with `--auth` so `nak` answers the challenge on rejection and retries:

```bash
nak event -c "hello relay" --auth --sec "$NOSTR_SECRET_KEY" ws://127.0.0.1:8080
```

Read the stored event back through an authenticated subscription:

```bash
nak req -k 1 -l 5 --force-pre-auth --sec "$NOSTR_SECRET_KEY" ws://127.0.0.1:8080
```

## Run the full Postgres gate

`./scripts/test-postgres.sh` provisions its own disposable cluster, runs the
store, gateway, multiprocess, import, and release-load suites, then boots the
compiled binary, checks `/health` and NIP-11, drives the acceptance client,
and verifies a shadow-read against a second process.

## Stop

```bash
pg_ctl -D "$pgdir/data" -m immediate stop
rm -rf "$pgdir"
```
