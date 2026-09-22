# Decision service discovery

The `decision-advertise` binary publishes the decision service's NIP-CAP
capability manifest — a `kind:30180` addressable event — to a relay. The
manifest is the public discovery document the NIP-CAP decision-service
contract defines: the lanes the serving path answers on, the doors an
unauthenticated caller may name, the limits it enforces, and the schema
versions it speaks.

## What the manifest claims

```json
{
  "interface": "openagents.systemone.v1",
  "service": {
    "lanes": [
      {"transport": "http", "endpoint": "https://gateway.example",
       "call": "/v1/systemone", "models": "/v1/models"},
      {"transport": "nostr-cj", "worker": "<worker pubkey hex>",
       "relays": ["wss://relay.example"],
       "request_kind": 25910, "result_kind": 26910, "feedback_kind": 27010}
    ],
    "doors": [
      {"name": "shared-kev", "model": "kev-0.6b",
       "artifact_signature": "sha256:…"}
    ],
    "limits": {"max_questions": 256, "request_window_seconds": 600},
    "versions": {"request": "openagents.systemone.v1",
                 "receipt": "openagents.receipt.execution.v1"}
  }
}
```

Doors come from the tenancy registry's `shared` set — the tool reads the
registry and copies name, model, and artifact signature. Tenant bindings,
quotas, workspaces, and credentials cannot reach the document; a door a
credential alone may name stays out of public discovery by construction.
Lanes come from the operator's own configuration, so the manifest claims
only what the operator configured the serving path to run.

## What it does not claim

A signed `30180` is an identity claim, not authorization. Discovery
answers "who says they serve this"; it does not admit a caller, bypass a
quota, or prove the lanes answer. A client resolves the manifest under an
operator-provisioned pin — the expected publisher pubkey and `d` slug —
then checks what it found at use:

- The `nostr-cj` lane's `worker` is the pubkey the caller `p`-tags and
  the signer every answer must carry; the decision-job family's binding
  checks enforce both.
- A result's sealed receipt names `served.model` and
  `served.artifact_signature`; a caller compares them to the discovered
  door entry and treats a mismatch as a fault in the serving path.
- The newest valid event per `(publisher, kind, d)` is the
  advertisement. A manifest outside the client's freshness window, ahead
  of its clock skew, or past a NIP-40 `expiration` refuses as stale — an
  older advertisement does not revive.

`nostr::cap::resolve_service` implements the client half: pin check,
signature and structure validation, freshness, replacement, and the
definition's own parse. `ServiceTrust` is the pin.

## Configuration

One `decision-advertise.json`:

```json
{
  "relay": "wss://relay.example",
  "service_secret": "<64 lowercase hex>",
  "slug": "decision-edge",
  "package": "openagents",
  "summary": "The decision service at the edge.",
  "endpoint": "https://gateway.example",
  "worker": "<worker pubkey hex>",
  "worker_relays": ["wss://relay.example"],
  "registry": "/var/lib/openagents/registry",
  "limits": {"max_questions": 256, "request_window_seconds": 600},
  "input": {"digest": "sha256:…", "size": 0, "media_type": "application/schema+json"},
  "output": {"digest": "sha256:…", "size": 0, "media_type": "application/schema+json"},
  "expiration_seconds": 86400
}
```

- `service_secret` may be omitted when `DECISION_ADVERTISE_SECRET` is
  set. The signer is the identity clients pin — keep it stable across
  republishes, and keep it out of every file the manifest itself could
  reach.
- At least one of `endpoint` (the HTTP lane) or `worker` (the NIP-CJ
  lane) is required. `worker_relays` defaults to `relay`.
- `input` and `output` are the request and response schema references —
  ArtifactRefs (`digest`, `size`, `media_type`) for the envelope schemas
  the lanes speak.
- `expiration_seconds` adds a NIP-40 `expiration` tag; republish inside
  the window or clients resolve the advertisement as stale.

## Running

```console
decision-advertise decision-advertise.json
```

The tool authenticates to the relay with NIP-42, publishes the event,
and waits for the relay's `OK`. A `false` verdict or a closed socket is
a nonzero exit. Republishing under the same `(publisher, kind, d)`
replaces the previous manifest — the addressable-event rule — so a
limits or lane change takes effect with one run.

## Discovery from a client

```text
REQ {"kinds": [30180], "authors": ["<publisher>"], "#d": ["decision-edge"]}
```

Then resolve the returned events under the pin:

```rust
let manifest = nostr::cap::resolve_service(&events, &trust, now)?;
let lane = manifest.contract.relay_lane();      // or the http lane
let door = manifest.contract.door("shared-kev");
```

The result's `receipt.served.model` and `receipt.served.artifact_signature`
must equal `door.model` and `door.artifact_signature`. Anything else is an
identity fault in the serving path, not an answer to relabel.
