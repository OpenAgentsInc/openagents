# Nexus Production Cutover

Date: March 6, 2026
Issue: `#3049`

## Scope

Move `nexus.openagents.com` from the old stateless public path to the durable Nexus service running on the production VM.

Production target for this pass:

- VM: `nexus-mainnet-1`
- project: `openagentsgemini`
- zone: `us-central1-a`
- image: `us-central1-docker.pkg.dev/openagentsgemini/openagents-nexus/nexus-relay:9ebeb4be4996`
- data disk: `nexus-relay-data-mainnet`
- public hostname: `nexus.openagents.com`
- tunnel: `nexus-mainnet`

## What changed

1. Built and pushed the production Nexus image.
2. Provisioned `nexus-mainnet-1` with the persistent relay disk attached.
3. Deployed the durable single-service Nexus runtime on the VM.
4. Created the Cloudflare tunnel `nexus-mainnet`.
5. Routed `nexus.openagents.com` to that tunnel.
6. Installed `nexus-cloudflared.service` on the VM so the hostname now fronts the private durable VM.

## Production runtime result

The production VM is now the authoritative Nexus runtime:

- `nexus-relay.service` is active
- `nexus-cloudflared.service` is active
- durable relay data lives at `/var/lib/nexus-relay`
- the data disk is mounted on the VM as persistent ext4 storage

Deploy receipt:

- `docs/reports/nexus/20260306-221610-deploy-receipt.json`

## Public validation

### Hostname and edge

- `dig +short nexus.openagents.com`
  - resolved to Cloudflare edge IPs after cutover
- `curl -I https://nexus.openagents.com/`
  - returned `server: cloudflare`
  - returned `200 OK`

### Durable relay health

- `curl https://nexus.openagents.com/healthz`
  - `relay_backend = durable-upstream`
  - `authority_mode = in-process`
  - `managed_groups_mode = deferred`
  - `data_directory = /var/lib/nexus-relay`

### Authority/API path

- `curl https://nexus.openagents.com/api/stats`
  - `service = nexus-control`
  - `hosted_nexus_relay_url = wss://nexus.openagents.com/`
  - `receipt_persistence_enabled = true`

This is the key public-state change from the prior hostname path, which reported `receipt_persistence_enabled = false`.

### Relay protocol checks

- `GET /` with `Accept: application/nostr+json`
  - returned NIP-11 info branded as `OpenAgents Nexus`
- websocket connect to `wss://nexus.openagents.com/`
  - returned an `AUTH` challenge frame immediately

### Public publish + replay

Using the known-good sample event from the durable relay tests:

- publish response included:
  - `OK`
  - `duplicate`

The duplicate response is expected because the same sample event had already been persisted during validation.

- replay request returned:
  - `AUTH`
  - `EVENT`
  - `EOSE`

### Restart persistence through the public hostname

After restarting `nexus-relay.service` on `nexus-mainnet-1`:

- the service returned to `active`
- `https://nexus.openagents.com/healthz` stayed healthy
- replay through `wss://nexus.openagents.com/` returned the same persisted sample event
- replay completed with `EOSE`

This confirms the public hostname now fronts durable storage that survives service restart.

## Conclusion

`nexus.openagents.com` is now serving the durable Nexus service.

Acceptance criteria met:

- `nexus.openagents.com` serves the durable Nexus service
- live relay and HTTP API traffic are healthy
- persistence survives restart through the public hostname

## Follow-up cleanup

Still to do in the next issue:

- remove the old in-memory relay paths from the repo
- remove no-longer-needed transitional public Cloud Run wiring
- update docs/runbooks so the durable VM + tunnel path is canonical
