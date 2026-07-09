# CND-038 Redacted Config And Environment Management

> **Historical bootstrap note (#8591).** Kept for archaeology and ops memory.
> Active Cloud implementation is in the public monorepo (`crates/*`,
> `docs/cloud/`). Deprecated authority names: **Vortex** → Worker/Khala Sync;
> **Treasury product** → Worker credits + MDK/Nexus payout bridge only;
> **Nexus-as-registry** → Worker/Khala Sync (CLI may still say `nexus`).
> Do not treat this note as current product-authority ownership.

Status: implemented scaffold

This issue establishes the first local and GCP config convention for managed
OpenAgents Cloud nodes and workrooms.

## Tracked Templates

Tracked examples live under `config/`:

- `config/oa-node.env.example`
- `config/oa-workroomd.env.example`
- `config/gcp-node.env.example`

These files contain only non-secret defaults and server-side secret references
such as `gcp-secret://...`. They must not contain raw provider tokens, Codex
auth JSON, wallet material, private keys, bearer tokens, passwords, or private
fleet topology.

## Private Overlays

Operators should copy templates into ignored local files before use:

```bash
cp config/oa-node.env.example .secrets/oa-node.env
cp config/oa-workroomd.env.example .secrets/oa-workroomd.env
cp config/gcp-node.env.example .secrets/gcp-node.env
```

The repo ignores `.env`, `.env.*`, `.secrets/`, and `*.local.env`. GCP runtime
secrets belong in Secret Manager and should enter scripts as secret references,
not raw values.

## Redaction Verification

Run:

```bash
scripts/verify-redacted-config.sh
```

The verifier builds representative env, URL, headers, config, log, and receipt
fixtures with fake secret markers, runs each through:

```bash
cargo run -p oa-node -- broker redact --kind <kind> --input <file> --json
```

and fails if any redacted artifact or broker receipt still contains secret-like
material. The fixture is marked with `OPENAGENTS_FAKE_SECRET_OK` so the broker
can prove redaction behavior without accepting real secrets.

## GCP Boundary

GCP bootstrap and deploy scripts may accept project, region, zone, node name,
image tag, and environment name. They must not accept raw provider secrets as
arguments. When a test needs credential material, it should use a bounded Secret
Manager placeholder and document the exact service account that can access it.

## Required Behavior

- Local config is templated and tracked only as `.example`.
- Private local overlays stay outside Git.
- GCP secrets are referenced, not embedded.
- Logs and receipts use `oa-node broker redact` before persistence or
  projection when they may include env, URL, headers, config, log, or receipt
  payload content.
- Redaction receipts are digest-only and point at redacted artifacts.
