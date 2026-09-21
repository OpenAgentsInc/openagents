# Runbook: Debian VPS (canonical single-box deployment)

This is the canonical Immortal deployment: Debian 13 (`trixie`), Postgres 17
from apt, one `nostr-relay` binary under systemd, and Caddy or nginx terminating
public TLS. It applies to a physical server or any VPS provider. All durable
event/control state stays in the one Postgres database; M7's content-addressed
media bytes stay under `/var/lib/nostr-relay/media` and their visibility remains
Postgres-owned.

The committed files under `deploy/` are the source of truth for service,
proxy, environment, and backup configuration. Do not maintain private copies
of the snippets in this document.

Replace these placeholders before enabling the service:

- `relay.example.com` — the relay's public DNS name;
- `<YOUR_DB_PASSWORD>` — a long random database password; and
- `<VERSION>` — the immutable release label installed on this server.

Prerequisites:

- a fresh Debian 13 amd64 or arm64 server with root or sudo access;
- DNS `A` and, if applicable, `AAAA` records pointing at the server; and
- either a release binary built for the server or a repository checkout from
  which to build it.

## 1. Base system

```sh
sudo apt-get update
sudo apt-get upgrade -y
sudo apt-get install -y postgresql curl ca-certificates
```

If building on the server, install Debian's Rust 1.85 toolchain and compiler:

```sh
sudo apt-get install -y cargo build-essential
cargo build --locked --release
```

Allow only SSH, HTTP, and HTTPS at the provider firewall. If the provider has
no firewall, use `ufw`:

```sh
sudo apt-get install -y ufw
sudo ufw allow OpenSSH
sudo ufw allow 80/tcp
sudo ufw allow 443/tcp
sudo ufw enable
```

The relay binds `127.0.0.1:8080`; never expose that port publicly.

## 2. Postgres

Debian's packaged Postgres starts automatically and listens locally. Create a
plain login role and one database without putting the password in shell
history:

```sh
sudo -u postgres createuser --pwprompt nostr-relay
sudo -u postgres createdb --owner=nostr-relay nostr-relay
```

Verify the credential:

```sh
psql 'postgres://nostr_relay:<YOUR_DB_PASSWORD>@127.0.0.1:5432/nostr-relay' \
  --command='SELECT 1;'
```

The `nostr-relay` role is not a superuser and owns only its database. The first
relay start applies the embedded schema under an advisory lock and records its
hash; do not apply files under `migrations/` directly with `psql`.

## 3. Install the binary

Use an immutable release directory and one atomic `current` symlink:

```sh
sudo useradd --system --home /nonexistent --shell /usr/sbin/nologin nostr-relay
sudo install -d -o root -g root -m 0755 /opt/nostr-relay/releases/<VERSION>
sudo install -o root -g root -m 0755 nostr-relay \
  /opt/nostr-relay/releases/<VERSION>/nostr-relay
sudo ln -sfn /opt/nostr-relay/releases/<VERSION> /opt/nostr-relay/current
```

When building from this checkout, the source path is
`target/release/nostr-relay` instead of `nostr-relay`.

## 4. Configure the environment

Install the committed template, then replace its password, hostname, and any
operator-specific values with `sudoedit`:

```sh
sudo install -d -o root -g nostr-relay -m 0750 /etc/nostr-relay
sudo install -o root -g nostr-relay -m 0640 deploy/nostr-relay.env.example \
  /etc/nostr-relay/nostr-relay.env
sudoedit /etc/nostr-relay/nostr-relay.env
if sudo grep -q '<' /etc/nostr-relay/nostr-relay.env; then
  echo 'ERROR: unresolved placeholder remains' >&2
  false
fi
```

The database password lives only in this root-owned file. Never put it in the
unit, command line, repository, or logs. The complete environment contract is
in [`configuration.md`](configuration.md).

## 5. Install the hardened systemd unit

```sh
sudo install -o root -g root -m 0644 deploy/systemd/nostr-relay.service \
  /etc/systemd/system/nostr-relay.service
sudo systemd-analyze verify /etc/systemd/system/nostr-relay.service
sudo systemctl daemon-reload
sudo systemctl enable --now nostr-relay.service
```

Verify startup and inspect the sandbox:

```sh
systemctl status nostr-relay.service --no-pager
curl -fsS http://127.0.0.1:8080/health
curl -fsS -H 'Accept: application/nostr+json' http://127.0.0.1:8080/
sudo systemd-analyze security --no-pager nostr-relay.service
journalctl -u nostr-relay.service -n 20 --no-pager
```

The canonical unit permits only localhost networking and port 8080, permits
writes only to its private `/var/lib/nostr-relay` state directory, removes
capabilities, restricts namespaces and system calls, and stops within 15
seconds. Change the socket or filesystem restrictions only if you also change
the documented single-box topology.

## 6. Put a TLS reverse proxy in front

Pick one. Both templates preserve WebSocket upgrades and the client-address
headers used when `NOSTR_RELAY_TRUST_PROXY=true`.

### Caddy (recommended)

```sh
sudo apt-get install -y caddy
sudo install -o root -g root -m 0644 deploy/caddy/Caddyfile \
  /etc/caddy/Caddyfile
sudoedit /etc/caddy/Caddyfile
sudo caddy validate --config /etc/caddy/Caddyfile
sudo systemctl reload caddy.service
```

Caddy obtains and renews the public certificate automatically.

### nginx

```sh
sudo apt-get install -y nginx certbot python3-certbot-nginx
sudo install -o root -g root -m 0644 deploy/nginx/nostr-relay.conf \
  /etc/nginx/sites-available/nostr-relay
sudoedit /etc/nginx/sites-available/nostr-relay
sudo ln -sfn /etc/nginx/sites-available/nostr-relay \
  /etc/nginx/sites-enabled/nostr-relay
sudo nginx -t
sudo systemctl reload nginx.service
sudo certbot --nginx -d relay.example.com --redirect
```

The 600-second proxy timeouts are inactivity timeouts; normal WebSocket ping
traffic keeps a connection open.

Verify from outside the server:

```sh
curl -fsS -H 'Accept: application/nostr+json' \
  https://relay.example.com/
```

Then connect a Nostr client to `wss://relay.example.com`, publish an event,
and query it back.

## 7. Install and prove nightly backups

The backup service writes one restorable unit per run: a private custom-format
`pg_dump`, a media tar archive that holds every blob the dump references, and
a manifest that names both files with their SHA-256 digests and sizes. The
manifest is the last file written and appears only after the script has
confirmed that every ready `media_blob` row in the dump has its bytes in the
archive. A dump or archive without a manifest is a failed run, not a restore
point, and the next run removes it. Units are retained 14 days locally and the
timer catches up after downtime. Install the committed artifacts:

```sh
sudo install -d -o postgres -g postgres -m 0700 /var/backups/nostr-relay
sudo install -o root -g root -m 0755 deploy/backup/nostr-relay-backup \
  deploy/backup/nostr-relay-restore /usr/local/sbin/
sudo install -o root -g root -m 0644 \
  deploy/backup/nostr-relay-backup.service \
  deploy/backup/nostr-relay-backup.timer \
  /etc/systemd/system/
sudo systemd-analyze verify \
  /etc/systemd/system/nostr-relay-backup.service \
  /etc/systemd/system/nostr-relay-backup.timer
sudo systemctl daemon-reload
sudo systemctl enable --now nostr-relay-backup.timer
sudo systemctl start nostr-relay-backup.service
sudo systemctl status nostr-relay-backup.service --no-pager
sudo ls -l /var/backups/nostr-relay/
sudo cat /var/backups/nostr-relay/nostr-relay-<TIMESTAMP>.manifest
```

Copy whole units (all three files) off the server on an operator-controlled
schedule. A dump on the same disk is a restore point, not a disaster-recovery
backup.

The timer runs online. The unit is consistent because the relay never destroys
a deleted blob's bytes at once: a delete moves the file to
`<media root>/.deleted/`, where it stays for 48 hours, and the script dumps the
database before collecting its referenced blobs. The collector copies each
blob from its live path or, if deletion moved it, its retained path. It checks
each content digest, then archives the staging directory. This avoids missing
a blob renamed between directory visits. Unreferenced blobs and unfinished
uploads do not enter the archive. Allow space for the staged media and archive
at the same time. The relay removes retained files after their window; finish
collection within that window and do not clear `.deleted/` by hand while a
backup is due.

Test the newest unit immediately, into an empty database and an empty media
root. The restore script checks both digests against the manifest before it
writes, refuses a target that already holds tables or files, moves bytes the
archive holds only under `.deleted/` back to the live path when the dump still
names the blob, and fails if any ready blob row ends up without bytes:

```sh
sudo -u postgres createdb --owner=nostr-relay nostr_relay_restore_test
sudo -u postgres nostr-relay-restore \
  /var/backups/nostr-relay/nostr-relay-<TIMESTAMP>.manifest \
  nostr_relay_restore_test /var/tmp/nostr-relay-restore-test
sudo -u postgres psql --dbname=nostr_relay_restore_test \
  --command='SELECT count(*) FROM nostr_event;'
sudo -u postgres psql --dbname=nostr_relay_restore_test \
  --command='SELECT version, name, sha256 FROM schema_migrations ORDER BY version;'
sudo -u postgres dropdb nostr_relay_restore_test
sudo rm -rf /var/tmp/nostr-relay-restore-test
```

The same check runs against a disposable relay in
`crates/nostr-relay/tests/backup_postgres.rs` under
`./scripts/test-postgres.sh`, with uploads and deletes in flight while the
backup runs.

### Recover the production database

Keep the failed database and media root until the restored relay passes
verification:

```sh
sudo systemctl stop nostr-relay.service
sudo -u postgres psql --dbname=postgres \
  --command="SELECT pg_terminate_backend(pid) FROM pg_stat_activity WHERE datname = 'nostr_relay';"
sudo -u postgres psql --dbname=postgres \
  --command='ALTER DATABASE nostr_relay RENAME TO nostr_relay_failed;'
sudo mv /var/lib/nostr-relay/media /var/lib/nostr-relay/media.failed
sudo -u postgres createdb --owner=nostr-relay nostr_relay
sudo -u postgres nostr-relay-restore \
  /var/backups/nostr-relay/nostr-relay-<TIMESTAMP>.manifest \
  nostr_relay /var/lib/nostr-relay/media
sudo chown -R nostr-relay:nostr-relay /var/lib/nostr-relay/media
sudo chmod -R u=rwX,g=rX,o= /var/lib/nostr-relay/media
sudo systemctl start nostr-relay.service
curl -fsS http://127.0.0.1:8080/health
journalctl -u nostr-relay.service -n 30 --no-pager
```

After publish/query verification and owner approval, remove
`nostr_relay_failed` and `media.failed`. If verification fails, stop the
service, drop the restored `nostr_relay`, rename `nostr_relay_failed` back to
`nostr_relay`, move `media.failed` back, and start the service.

For a tighter recovery-point objective, configure Postgres WAL archiving and
periodic base backups to operator-controlled off-host storage. That remains
one Postgres; it does not add a product service.

## 8. Upgrade

Take and verify a cold paired backup before changing the binary:

```sh
sudo systemctl stop nostr-relay.service
sudo systemctl start nostr-relay-backup.service
sudo systemctl status nostr-relay-backup.service --no-pager
sudo -u postgres psql --dbname=nostr_relay \
  --command='SELECT version, name FROM schema_migrations ORDER BY version;'
```

Stage the new release beside the current one, flip the symlink, restart, and
verify:

```sh
sudo install -d -o root -g root -m 0755 \
  /opt/nostr-relay/releases/<NEW_VERSION>
sudo install -o root -g root -m 0755 nostr-relay \
  /opt/nostr-relay/releases/<NEW_VERSION>/nostr-relay
sudo ln -sfn /opt/nostr-relay/releases/<NEW_VERSION> /opt/nostr-relay/current
sudo systemctl restart nostr-relay.service
curl -fsS http://127.0.0.1:8080/health
journalctl -u nostr-relay.service -n 30 --no-pager
```

On SIGTERM the relay stops accepting connections, drains in-flight admission
within `NOSTR_RELAY_SHUTDOWN_GRACE_SECONDS`, and exits. It never sends `OK`
before commit.

## 9. Roll back

Read the release notes before relying on a binary-only rollback. An older
binary deliberately rejects an unknown migration version, so a release that
applied a new migration requires the pre-upgrade database restore as well as
the old binary.

If the failed release applied no migration, flip back directly:

```sh
sudo ln -sfn /opt/nostr-relay/releases/<OLD_VERSION> /opt/nostr-relay/current
sudo systemctl restart nostr-relay.service
curl -fsS http://127.0.0.1:8080/health
```

If it applied a migration, follow **Recover the production database** with
the pre-upgrade dump while the old binary is selected. This fail-closed rule
prevents an old release from silently interpreting a schema it does not know.

## 10. Routine checks

- `systemctl is-active nostr-relay.service nostr-relay-backup.timer`
- `journalctl -u nostr-relay.service --since=-1h -p warning --no-pager`
- `curl -fsS https://relay.example.com/health` from off-host
- `df -h /var/lib/postgresql /var/backups/nostr-relay`
- `sudo -u postgres psql --dbname=nostr_relay --command="SELECT pg_size_pretty(pg_database_size('nostr-relay'));"`
- `systemctl list-timers nostr-relay-backup.timer --no-pager`
- restore the newest off-host dump into a temporary database at least monthly

## 11. Reproduce the fresh-Debian acceptance

The guarded acceptance command starts a disposable Debian 13 container,
installs apt Postgres and Debian Rust, builds the release binary, serves
health and NIP-11, publishes and reads a pinned signed event, then creates and
restores a logical backup:

```sh
./scripts/run-debian-acceptance.sh
```

It uses a running Apple Container, Podman, or Docker runtime selected locally
and requires a wrapper-only disposable-container guard before the destructive
inner script runs. It does not use GitHub workflows or any GitHub-billed
service.

## Current platform references

- [Debian releases](https://www.debian.org/releases/)
- [Debian 13 release information](https://www.debian.org/releases/trixie/)
