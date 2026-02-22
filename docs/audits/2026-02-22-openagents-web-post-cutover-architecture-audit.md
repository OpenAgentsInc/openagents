# OA-WEBPARITY-061 Post-Cutover Architecture Audit

Date: 2026-02-22
Scope: `apps/openagents.com` Laravel-to-Rust/WGPUI parity program
Tracking issue: `OA-WEBPARITY-061` (`#2013`)

## Executive Snapshot

- Parity issue set size: `68`
- Closed: `60`
- Open: `8` (`#2013` to `#2020`)
- Active serving topology: Rust-only (`openagents-control-service` + Rust web shell)

Evidence basis:
- `apps/openagents.com/docs/20260222-oa-webparity-056-full-regression-suite.md`
- `apps/openagents.com/docs/20260222-oa-webparity-057-staging-dual-run-shadow-report.md`
- `apps/openagents.com/docs/20260222-oa-webparity-058-production-canary-rollback-drill.md`
- `apps/openagents.com/docs/20260222-oa-webparity-059-production-rust-route-flip.md`
- `apps/openagents.com/docs/20260222-oa-webparity-060-retire-laravel-serving-path.md`

## What Is Complete

1. Rust-only serving lane is enforced and legacy deploy lane is archived.
2. Production route flip controls exist and API paths are pinned to Rust authority, including under legacy override conditions.
3. Full parity regression lane is implemented and wired in CI.
4. Canary/rollback drill automation exists with structured artifact output.

## Residual Risks / Gaps

1. Staging dual-run report currently includes local rehearsal evidence; canonical staging environment URLs were unavailable at execution time.
2. Canary/rollback and route-flip “live apply” commands are implemented but still require operator-run execution in production windows.
3. Cross-cutting parity debt remains in OA-WEBPARITY-062 through OA-WEBPARITY-068.

## Outstanding Debt Register (Open Issues)

- `#2014` OA-WEBPARITY-062: non-HTTP behavior parity (cookies/CORS/cache/throttle/WS handshake).
- `#2015` OA-WEBPARITY-063: auth/session edge-case parity (refresh race, revoke/logout-other-devices, guest retirement).
- `#2016` OA-WEBPARITY-064: webhook signature/idempotency/replay parity.
- `#2017` OA-WEBPARITY-065: static asset/compression/service-worker delivery parity.
- `#2018` OA-WEBPARITY-066: queue/scheduler/listener parity and Laravel scheduler shutdown.
- `#2019` OA-WEBPARITY-067: mixed-version deploy, rollback, and backfill invariants.
- `#2020` OA-WEBPARITY-068: Rust-only terminal gate for remaining PHP/TypeScript implementation lanes.

## SLO and Observability Hardening Backlog

1. Promote canary log probes to scheduled automation, not only manual runbook execution.
2. Publish explicit control-service SLO dashboard with request success rate and p95 latency for:
   - `/healthz`
   - `/api/auth/verify`
   - `/api/sync/token`
   - route-shell load path (`/` + static assets)
3. Add alerting for sustained route-split anomalies:
   - unexpected `legacy` target decisions
   - spike in `legacy_route_unavailable` errors
4. Add an automated post-deploy assertion that all sampled `/api/*` evaluate probes return `rust_shell`.
5. Add production evidence capture policy: every cutover command run must persist artifact paths in the owning issue comment.

## Parity Program Exit Conditions (Remaining)

1. Close OA-WEBPARITY-062 through OA-WEBPARITY-068 with linked verification artifacts.
2. Record one successful staging dual-run execution against live staging hosts.
3. Record one successful production operator execution for:
   - route-flip apply (`APPLY=1`)
   - canary/rollback live drill
4. Complete post-cutover observation window with stable SLOs and no critical parity regressions.
