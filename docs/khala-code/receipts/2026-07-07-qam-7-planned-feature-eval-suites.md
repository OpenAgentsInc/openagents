# QAM-7 Planned-Feature Eval Suites Receipt

Date: 2026-07-07

Issue: #8542

Status: FIXTURE-FIRST SUITES AUTHORED RED/WAIVED

This receipt records the planned-feature eval-suite catalog required before
post-MVP implementation lanes begin. These suites are intentionally red/waived:
they define acceptance and blockers before code exists.

## Shipped

- Added typed catalog:
  `clients/khala-mobile/src/qa/planned-feature-eval-suites.ts`.
- Added oracle test:
  `clients/khala-mobile/tests/planned-feature-eval-suites.test.ts`.
- Added behavior contract:
  `khala_mobile.qa.planned_feature_eval_suites_fixture_first.v1`.

## Authored Suites

- Sarah SR-1..3:
  - qualification flow
  - discount-pressure probe
  - injection-bearing email fixture
  - fake checkout close path
- IAP/minerals:
  - StoreKitTest purchase boundary (waived until device-tier IAP is armed)
  - server receipt validation / restore / clawback replay
  - Apple 3.1.1 copy oracle
- Push E2E:
  - `simctl push` notification to Khala thread deep-link navigation
- Codex connect CX-2:
  - device-auth state machine
  - account readiness/quota rendering
  - `account_exhausted` / `account_rate_limited` typed failures
- Agents panel AE-2:
  - `agents_panel.run_status_indicators_truthful.v1` fixture oracle

## Exit Interpretation

#8542 can close when the catalog and tests are merged: future implementation
PRs must turn these blocked/waived cases green rather than writing acceptance
criteria after the fact.

## Verification

- PASS: `bun test tests/planned-feature-eval-suites.test.ts tests/ux-contracts.test.ts`
  in `clients/khala-mobile`
  - 17 pass, 0 fail
- PASS: `bun run --cwd clients/khala-mobile typecheck`
- PASS: `bun run --cwd clients/khala-mobile qa:mobile:gate`
  - TypeScript pass
  - Dependency Cruiser pass
  - 400 pass, 0 fail
  - receipt: `clients/khala-mobile/var/qa-mobile-gate/khala-mobile-release-gate.latest.json`
- BLOCKED OUT OF SCOPE: `bun run check:deploy`
  - Existing architecture budget failures remain in Worker and desktop/tooling
    surfaces outside the QAM-7 edit scope, including:
    - `workers/api/src/khala-cloud-runtime-dispatch.ts`
    - `workers/api/src/khala-cloud-runtime-inference-block.ts`
    - `workers/api/src/khala-cloud-runtime-execution-token.ts`
    - `workers/api/src/khala-cloud-runtime-dispatch-admin-route.ts`
    - `workers/api/src/cloud/cloud-coding-session-routes.ts`
