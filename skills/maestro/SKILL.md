---
name: maestro
description: Maestro Symphony blockchain query operations for OpenAgents agents, including tip freshness checks, address/UTXO/runes queries, and production-safe troubleshooting against deployed Symphony API endpoints.
metadata:
  oa:
    project: maestro
    identifier: maestro
    version: "0.1.0"
    expires_at_unix: 1798761600
    capabilities:
      - http:outbound
      - filesystem:read
      - process:spawn
---

# Maestro

## Overview

Use this skill when an agent needs to query a deployed Maestro Symphony API for Bitcoin chain/index data, validate freshness against a backend bitcoind tip, and run safe operational checks without exposing secrets.

This skill assumes OpenAgents GCP deployment conventions from `docs/deploy/SYMPHONY_GCP_RUNBOOK.md`.

## Inputs

Set these env vars before querying:

- `SYMPHONY_BASE_URL`: Symphony API base URL.
- `SYMPHONY_NETWORK`: expected network (`mainnet`, `testnet4`, or `regtest`).
- `BITCOIND_RPC_URL`: backend RPC endpoint.
- `BITCOIND_RPC_USER` / `BITCOIND_RPC_PASS`: backend RPC credentials.

Run preflight first:

```bash
skills/maestro/scripts/check-symphony-prereqs.sh
```

## Workflow

1. Verify API liveness and parse tip.
2. Validate chain freshness by comparing against bitcoind height.
3. Run address/runes query endpoints.
4. Enforce safety controls (`/dump` restricted, no secret logging, bounded polling).

## Quick commands

```bash
curl -fsS "${SYMPHONY_BASE_URL}/tip" | jq .

ADDR="bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh"
curl -fsS "${SYMPHONY_BASE_URL}/addresses/${ADDR}/tx_count" | jq .
curl -fsS "${SYMPHONY_BASE_URL}/addresses/${ADDR}/utxos" | jq .
curl -fsS "${SYMPHONY_BASE_URL}/addresses/${ADDR}/runes/balances" | jq .
```

## References

- [symphony-query-recipes](references/symphony-query-recipes.md)
