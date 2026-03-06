# Nexus Staging Validation

Date: March 6, 2026
Issue: `#3048`

## Scope

Validate the durable single-service Nexus staging deployment before any public cutover.

Staging access in this pass is private:

- VM: `nexus-staging-1`
- project: `openagentsgemini`
- zone: `us-central1-a`
- access path: `gcloud compute ssh --tunnel-through-iap`
- local staging relay URL during validation: `ws://127.0.0.1:18080/`

## Deployed image

- image: `us-central1-docker.pkg.dev/openagentsgemini/openagents-nexus/nexus-relay:872bbaa8bdaa`
- Cloud Build: `a1851efa-317b-41bc-b5d9-52460dc16452`

## Deployment notes

Two real build gaps were discovered and fixed during staging:

1. the Nexus image needed Rust `1.88` instead of `1.87`
2. the build stage needed `protobuf-compiler` so the vendored relay could compile `proto/nauthz.proto`

## Validation results

### Service health

- `nexus-relay.service` reached `active`
- `/healthz` returned:
  - `relay_backend = durable-upstream`
  - `authority_mode = in-process`
  - `managed_groups_mode = deferred`
  - `data_directory = /var/lib/nexus-relay`

### NIP-11

- `GET /` with `Accept: application/nostr+json` returned NIP-11 relay info from the durable upstream relay
- response remained branded as `OpenAgents Nexus`

### NIP-42

- opening a websocket to staging immediately returned an `AUTH` challenge frame
- example observed frame shape:
  - `["AUTH","<challenge>"]`

### Relay publish + replay

- published the known-good sample event used by the local durable relay tests
- received:
  - `["OK","f3ce6798d70e358213ebbeba4886bbdfacf1ecfd4f65ee5323ef5f404de32b86",true,""]`
- subscribed with `["REQ","stage-check",{}]`
- received:
  - `AUTH`
  - replayed `EVENT`
  - `EOSE`

### Restart persistence

- restarted `nexus-relay.service` on the staging VM
- repeated the same replay request after restart
- received the previously published event again followed by `EOSE`
- this confirms durable storage survived service restart on the persistent disk

### Authority/API path

- `POST /api/session/desktop` succeeded through the same staged Nexus process
- the session response returned:
  - `desktop_client_id = autopilot-desktop:staging-smoke`
  - `hosted_nexus_relay_url = ws://127.0.0.1:18080/`
- `GET /v1/kernel/snapshots/0` succeeded with the returned bearer token
- `GET /api/stats` showed:
  - `service = nexus-control`
  - `receipt_persistence_enabled = true`

## Conclusion

The private staging Nexus is live and passes the required validation slice:

- websocket relay flow works
- NIP-11 works
- NIP-42 challenge flow works
- publish + replay works
- persistence survives restart
- desktop session bootstrap works
- authority/API routes work through the same deployed process

This staging shape is ready for the next issue: deciding and executing the public production cutover path.
