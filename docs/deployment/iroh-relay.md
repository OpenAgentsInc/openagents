# iroh relay (iroh.openagents.com)

`iroh.openagents.com` is our [iroh](https://github.com/n0-computer/iroh)
relay. When two OpenAgents endpoints cannot reach each other directly or by
hole punching, their end-to-end encrypted QUIC packets go through it. It
also answers QUIC address discovery (QAD) on UDP 7842, which tells an
endpoint its public address. OpenAgents endpoints use this relay and leave
n0's public relays off; see
[auto-pairing, "Reachability and fallback"](../coder/design/2026-09-29-auto-pairing.md).

The relay sees only encrypted packets addressed by `EndpointId`. It holds no
credential of ours, and its state is its Let's Encrypt account and
certificate.

## What runs where

| Item | Value |
| --- | --- |
| Binary | `iroh-relay` 1.3.0, the upstream release `iroh-relay-v1.3.0-x86_64-unknown-linux-musl.tar.gz` (SHA-256 pinned in `install.sh`). Same version as the `iroh` crate our endpoints use. |
| VM | `oa-iroh-relay-1`, `e2-small`, Debian 12, `openagentsgemini` / `us-central1-a`, no service account, Shielded VM. |
| Address | Static `oa-iroh-relay-ip` = `34.136.30.163`. |
| DNS | Cloudflare `A iroh.openagents.com -> 34.136.30.163`, DNS only (not proxied), TTL 300. Cloudflare's proxy would break UDP 7842 and the relay's own ACME challenge. |
| Firewall | `iroh-relay-ingress` on tag `iroh-relay`: TCP 80, TCP 443, UDP 7842 from anywhere. SSH is the project's default rule. |
| Ports | `:80` HTTP relay and `/healthz`; `:443` HTTPS/WSS relay; UDP `:7842` QAD; `127.0.0.1:9090` Prometheus metrics. |
| TLS | `cert_mode = "LetsEncrypt"` inside the relay (TLS-ALPN-01 on 443). Cache in `/var/lib/iroh-relay/certs`; it renews itself. |
| Access | `access = "everyone"`, with 50 new connections per second (burst 200) and 4 MiB/s per client (burst 8 MiB). |
| Files | [`deploy/iroh-relay/`](../../deploy/iroh-relay): `iroh-relay.toml` -> `/etc/iroh-relay/iroh-relay.toml`, `iroh-relay.service` -> `/etc/systemd/system/`, binary under `/opt/iroh-relay/releases/v<version>` with a `current` symlink. |

It is not on Cloud Run because Cloud Run accepts only HTTP(S) and gRPC
inbound, so there would be no UDP 7842 for QAD.

## Deploy record

- 2026-09-29, [#9968](https://github.com/OpenAgentsInc/openagents/issues/9968):
  provisioned with [`deploy/iroh-relay/provision.sh`](../../deploy/iroh-relay/provision.sh)
  and installed with `install.sh`, iroh-relay 1.3.0. Let's Encrypt issued
  the certificate on first start (issuer `YE2`, valid to 2026-12-28). The
  commit that closes #9968 carries these files.

## Checks

```sh
curl -sS https://iroh.openagents.com/healthz
# {"status":"ok","version":"1.3.0","git_hash":"unknown"}
curl -sS -o /dev/null -w '%{http_code}\n' http://iroh.openagents.com/generate_204   # 204
```

The live tests connect two endpoints whose only transport is this relay
(IP transports cleared, no address lookup, no n0 relay), send 64 KiB through
it, and require every path of the connection to be a relay path. A second
test asks the relay for the caller's public address over QAD:

```sh
cargo test --manifest-path deploy/iroh-relay/live-test/Cargo.toml -- --ignored --nocapture
# QAD: udp_v4=true global_v4=Some(<your public ip>:<port>) ...
# path Relay(https://iroh.openagents.com/) rtt ~100ms
# test result: ok. 2 passed
```

Set `IROH_RELAY_URL` to point them at another relay. The crate is its own
workspace, so the root build does not compile iroh through it.

On the VM (`gcloud compute ssh oa-iroh-relay-1 --project openagentsgemini --zone us-central1-a`,
with the automation service account's `CLOUDSDK_CONFIG`):

```sh
systemctl status iroh-relay
journalctl -u iroh-relay -n 100 --no-pager
curl -s http://127.0.0.1:9090/metrics | grep -E '^relay_'
```

## Upgrade

Keep the relay on the same minor version as the `iroh` crate the endpoints
use.

1. Download `iroh-relay-v<version>-x86_64-unknown-linux-musl.tar.gz` from the
   [iroh release](https://github.com/n0-computer/iroh/releases) and take its
   SHA-256.
2. Check `iroh-relay.toml` against that version's `iroh-relay/src/main.rs`
   `Config` for renamed or new keys.
3. Update `VERSION` and `SHA256` in `install.sh`, and the `iroh` pin in
   `live-test/Cargo.toml`, in one commit.
4. Copy and install:

   ```sh
   cd deploy/iroh-relay
   gcloud compute scp --project openagentsgemini --zone us-central1-a \
     iroh-relay.toml iroh-relay.service install.sh oa-iroh-relay-1:/tmp/
   gcloud compute ssh oa-iroh-relay-1 --project openagentsgemini --zone us-central1-a \
     --command 'cd /tmp && sudo ./install.sh'
   ```

   `install.sh` verifies the checksum, installs the new release beside the
   old one, switches `current`, and restarts the unit. The certificate cache
   survives, so no new certificate is requested.
5. Run the checks above and add a line to the deploy record.

Rollback: on the VM, `sudo ln -sfn /opt/iroh-relay/releases/v<previous> /opt/iroh-relay/current && sudo systemctl restart iroh-relay`.

A config-only change is step 4 with the same version.

## Failure notes

- If the relay starts without a certificate, check that DNS still points at
  the static address and is not proxied, and that TCP 443 is open: the ACME
  challenge is answered on 443 by the relay itself. Let's Encrypt limits
  duplicate certificates to five per week, so keep `/var/lib/iroh-relay/certs`
  across reinstalls.
- The relay refuses `LetsEncrypt` mode without `contact`; the config uses
  `hostmaster@openagents.com`.
- If QAD fails but `/healthz` works, check the UDP 7842 firewall rule.
