# Khala Lightning Staging Cutover Drill

Date: 2026-02-20
Owner: Codex autonomous run
Scope: KHALA-027 staging evidence for API-only lightning-ops control-plane transport.

## Preconditions

- Laravel control-plane API endpoints available at `/api/internal/lightning-ops/control-plane/query|mutation`.
- `OA_LIGHTNING_OPS_SECRET` set on both `apps/openagents.com` and `apps/lightning-ops`.
- `apps/lightning-ops` Khala dependency removed; API/mock modes only.

## Verification commands

```bash
cd apps/lightning-ops
npm run typecheck
npm test
OA_LIGHTNING_OPS_API_BASE_URL=http://127.0.0.1:8099 OA_LIGHTNING_OPS_SECRET=test-ops-secret npm run smoke:compile -- --json
OA_LIGHTNING_OPS_API_BASE_URL=http://127.0.0.1:8099 OA_LIGHTNING_OPS_SECRET=test-ops-secret npm run smoke:security -- --json
OA_LIGHTNING_OPS_API_BASE_URL=http://127.0.0.1:8099 OA_LIGHTNING_OPS_SECRET=test-ops-secret npm run smoke:settlement -- --json
npm run smoke:full-flow -- --json
```

## Results

- `typecheck`: PASS
- `test`: PASS (`19` test files, `36` tests)
- `smoke:compile` (API): PASS
- `smoke:security` (API): PASS
- `smoke:settlement` (API): PASS
- `smoke:full-flow` (mock deterministic lane): PASS

## Cutover assertions

- Khala package removed from `apps/lightning-ops/package.json` and lockfile.
- Khala transport implementation removed (`src/controlPlane/khalaTransport.ts`).
- CLI and program modes accept only `api|mock` for control-plane smokes.
- Staging reconcile helper now targets API mode (`scripts/staging-reconcile.sh`).

## Artifacts

- Latest full-flow summary artifact generated under:
  - `output/lightning-ops/full-flow/smoke_full-flow_1771565610563/summary.json`
  - `output/lightning-ops/full-flow/smoke_full-flow_1771565610563/events.jsonl`
