# OA-RUST-065 WGPUI Web Shell Performance + Soak Signoff

Date: 2026-02-21
Status: pass
Issue: OA-RUST-065

## Scope

This signoff validates Rust web-shell baseline performance and stability for the active Rust-only web path (`openagents-control-service` + `openagents-web-shell`) before hard cutover completion.

Harness:

- Script: `apps/openagents.com/web-shell/scripts/perf-soak-signoff.sh`
- Artifact: `apps/openagents.com/web-shell/perf/signoff-20260221T161932Z.json`
- Latest pointer: `apps/openagents.com/web-shell/perf/latest.json`

## Budgets

- `root.p95_ms <= 120`
- `manifest.p95_ms <= 100`
- `wasm_asset.p95_ms <= 250`
- `auth_verify.p95_ms <= 250`
- `sync_token.p95_ms <= 250`
- `soak.errors == 0`
- `rss_growth_kb <= 51200`
- `js_asset_bytes <= 380000`
- `wasm_asset_bytes <= 5500000`
- `host_shim_bytes <= 12000`
- `wasm_gzip_bytes <= 1900000`

## Results

- Boot
  - Root: p95 `0.67ms`, avg `0.44ms`
  - Manifest: p95 `0.71ms`, avg `0.50ms`
  - WASM asset: p95 `2.13ms`, avg `1.91ms`
  - Asset sizes: js `133854B`, wasm `3079632B`, host shim `5242B`, wasm gzip `1179757B`
- Interaction
  - Auth verify: p95 `0.95ms`, avg `0.64ms`
  - Sync token: p95 `0.77ms`, avg `0.54ms`
  - Auth churn errors: `0`
- Soak (`180s`)
  - Manifest poll: p95 `0.86ms`, avg `0.65ms`
  - Errors: `0`
  - RSS: min `8704KB`, max `13888KB`, growth `5184KB`

All defined budgets passed.

## Notes

- This run is a local loopback baseline on `http://127.0.0.1:8787` and is intended as a deterministic regression gate for the Rust runtime path.
- Canonical budget/fallback spec: `apps/openagents.com/web-shell/docs/WASM_BOOT_PERFORMANCE_POLICY.md`.
- For release approval, pair this baseline with environment-level canary evidence in `apps/openagents.com/service/docs/CANARY_ROLLBACK_RUNBOOK.md`.

## Re-run

From repo root:

```bash
apps/openagents.com/web-shell/scripts/perf-soak-signoff.sh
```

Optional overrides:

```bash
SOAK_SECONDS=300 LATENCY_SAMPLES=120 AUTH_CHURN_SAMPLES=80 \
apps/openagents.com/web-shell/scripts/perf-soak-signoff.sh
```
