# QAM-5 Mobile Nightly Row Receipt

Date: 2026-07-07

Issue: #8540

Status: SCHEDULABLE FOUNDATION ONLY

This receipt records the QAM-5 nightly mobile row definition and its public-safe
owned-runner artifacts. It is not an exit receipt for #8540: seven consecutive
passed real nightly receipts do not exist yet, and QAM-4 Storybook V1 visual
capture remains blocked by #8539.

## Shipped

- Added typed QAM-5 row model in `apps/qa-runner/src/mobile-nightly.ts`.
- Added the owned Tailnet Mac launchd plist renderer and public example plist.
- Added QA Swarm projection metadata:
  `projection.qa_swarm.mobile.khala_code_nightly`.
- Added scheduled row steps for:
  - iOS simulator Maestro flows.
  - Seeded device monkey with screenshot-on-crash, memory/zombie oracle, and
    coverage ledger artifact refs.
  - QAM-4 visual capture.
  - Named mobile perf budgets.
  - Seam probes.
- Added strict failure-issue body generation for the mobile nightly row.
- Added the seven-consecutive-passed-receipts evaluator so the exit gate cannot
  pass early.
- Added behavior contract:
  `khala_mobile.qa.nightly_mobile_row_owned_runner_discipline.v1`.

## Required Perf Budgets

- `budget.khala_mobile.cold_launch.v1`
- `budget.khala_mobile.thread_switch.v1`
- `budget.khala_mobile.sync_bootstrap_to_live.v1`
- `budget.khala_mobile.ota_check_overhead.v1`

## Required Seam Probes

- `khala_sync_transport_live_classification`, required classification: `live`
- `mobile_session_bearer_bridge`

## Blockers / Waivers

- #8539: Storybook V1 full visual baseline set is not proven on a simulator yet.
- #8540: Seven consecutive passed nightly mobile receipts do not exist yet.

## Verification

- PASS: `bun test src/mobile-nightly.test.ts` in `apps/qa-runner`
  - 6 pass, 0 fail
- PASS: `bun run --cwd apps/qa-runner typecheck`
- PASS: `bun test tests/ux-contracts.test.ts` in `clients/khala-mobile`
  - 15 pass, 0 fail
- PASS: `bun run --cwd apps/qa-runner test`
  - 510 pass, 0 fail
- PASS: `bun run --cwd clients/khala-mobile qa:mobile:gate`
  - TypeScript pass
  - Dependency Cruiser pass
  - 397 pass, 0 fail
  - receipt: `clients/khala-mobile/var/qa-mobile-gate/khala-mobile-release-gate.latest.json`
- BLOCKED OUT OF SCOPE: `bun run check:deploy`
  - Existing architecture budget failures remain in Worker and desktop/tooling
    surfaces outside the QAM-5 edit scope, including:
    - `workers/api/src/khala-cloud-runtime-dispatch.ts`
    - `workers/api/src/khala-cloud-runtime-inference-block.ts`
    - `workers/api/src/khala-cloud-runtime-execution-token.ts`
    - `workers/api/src/khala-cloud-runtime-dispatch-admin-route.ts`
    - `workers/api/src/cloud/cloud-coding-session-routes.ts`
