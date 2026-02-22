# OA-WEBPARITY-056 Full Parity Regression Suite

Date: 2026-02-22
Status: pass
Issue: OA-WEBPARITY-056

## Scope

Create an enforceable parity regression lane that validates:
- Rust control-service API behavior
- Rust/WGPUI web-shell UI build lane
- Khala WS stream-contract behavior
- codex-only chat authority and legacy Vercel endpoint retirement semantics

## Delivered Harness

New regression script:
- `apps/openagents.com/scripts/run-full-parity-regression.sh`

Structured output artifacts:
- `apps/openagents.com/storage/app/parity-regression/<timestamp>/summary.json`
- `apps/openagents.com/storage/app/parity-regression/<timestamp>/SUMMARY.md`
- per-step logs under the same output directory

Local CI lane integration:
- `scripts/local-ci.sh web-parity`

CI workflow gate:
- `.github/workflows/web-parity-regression.yml`
- check name: `web-parity-regression / parity-regression`

## Regression Categories

1. API parity
- Service compile baseline
- Legacy `/api/chats*` alias behavior bound to codex thread authority (`legacy_chats_aliases_map_to_codex_threads`)

2. UI parity
- Web-shell wasm compile baseline

3. Stream parity
- Legacy stream alias retirement/bridge semantics
- Khala WS smoke contract metadata semantics

4. Internal codex authority parity
- Codex worker control write-path remains canonical (`runtime_codex_control_request_accepts_turn_start_and_persists_message`)

## Codex-Only Authority and Legacy Retirement Coverage

The suite explicitly enforces:
- legacy Vercel-style chat endpoints remain alias/retirement lanes (not separate authority)
- canonical write authority remains codex worker control/runtime thread APIs
- stream protocol in retirement path remains disabled (no SSE reintroduction)

## Verification

Executed:

```bash
cargo fmt --manifest-path apps/openagents.com/service/Cargo.toml
cargo check --manifest-path apps/openagents.com/service/Cargo.toml --bin openagents-control-ops
cargo test --manifest-path apps/openagents.com/service/Cargo.toml
./apps/openagents.com/scripts/run-full-parity-regression.sh
```

## Notes

- The regression script supports optional full-suite service execution via:
  - `OA_WEB_PARITY_FULL_SERVICE_TESTS=1 ./apps/openagents.com/scripts/run-full-parity-regression.sh`
- Step results are machine-readable (`summary.json`) for CI and audit ingestion.
