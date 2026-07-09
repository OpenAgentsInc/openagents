# oa-node Registry Adapter (historical CLI name: `nexus`)

Status: Cloud MVP scaffold for `CND-009` — **authority rewrite 2026-07-09 (#8591)**

> **Naming:** the CLI verbs remain `oa-node nexus register` / `oa-node nexus
> heartbeat` for wire compatibility with existing tests and ops muscle memory.
> They do **not** mean the deprecated Nexus product owns managed-node registry
> state. Current product registry / capacity projection lives on the
> **openagents.com Worker** and **Khala Sync** surfaces. The **MDK/Nexus bridge**
> remains active only for outbound payout/custody — not for node registry.

`oa-node nexus register` and `oa-node nexus heartbeat` connect the managed node
daemon scaffold to a registry HTTP surface (local or Worker-shaped).

```bash
oa-node nexus register --base-url http://127.0.0.1:8080 --json
oa-node nexus heartbeat --base-url http://127.0.0.1:8080 --json
```

The adapter posts to:

- `POST /v1/cloud/nodes/register`
- `POST /v1/cloud/nodes/heartbeat`

## Registry Envelope

The request body uses schema `openagents.oa_node.nexus_registry.v1` and carries:

- action: `register` or `heartbeat`;
- node id and org id from `node-state.json`;
- the current `openagents.cloud_node.v1` snapshot digest;
- observed status and desired mode labels;
- a signature block containing algorithm, signing-key reference, and digest.

The MVP signature is `sha256-ref-bound-mvp`: a deterministic digest over the
registry schema, action, node/org identity, snapshot digest, lifecycle labels,
and signing-key reference. It is intentionally key-reference-bound only. Raw
private key material is never written to local state, registry requests, docs,
or logs.

Production registry endpoints (Worker-backed or equivalent) should replace this
with a real key-backed signature while preserving the same invariant:
registration and heartbeat authority is bound to the same node identity and
snapshot digest the registry is accepting.

## Responses

Accepted response:

```json
{
  "status": "accepted",
  "desired_mode": "online",
  "registration_expires_at_ms": 9999999999999,
  "detail": "ok"
}
```

Rejected or stale response:

```json
{
  "status": "stale",
  "detail": "snapshot digest is stale"
}
```

`accepted` may update the persisted desired mode after validating it against the
contract enum. `rejected`, `stale`, `expired`, invalid HTTP responses, and
transport failures degrade the local admin store instead of crashing or
claiming capacity:

- `observed_status` becomes `degraded`;
- `last_degradation_reason` records a registry-specific reason;
- `health-events.jsonl` receives an append-only event such as
  `nexus_registration_rejected`, `nexus_registration_stale`, or
  `nexus_registration_expired` (event type strings keep the historical prefix).

This keeps the node locally inspectable and prevents unsafe assignment intake
when registry authority is missing, stale, or actively rejecting the node.

## Local Test Path

The integration tests in `crates/oa-node/tests/nexus_registry.rs` run a local
single-request HTTP server and verify that:

- registration posts a signed envelope with the snapshot digest;
- accepted registration can apply desired mode;
- rejected heartbeats degrade safely and emit a health event;
- stale registration degrades safely.

## Related

- Active Cloud architecture: `docs/cloud/ARCHITECTURE.md`
- Settlement mode labels (not payout authority): `docs/cloud/oa-node/SETTLEMENT_MODES.md`
- Outbound payout/custody: MDK/Nexus bridge (not this adapter)
