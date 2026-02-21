# Rust Migration Legacy Dependency Inventory (OA-RUST-002)

Status: Active baseline
Last updated: 2026-02-21
Owner lane: `owner:contracts-docs` (interim DRI `@AtlantisPleb`)
Source issue: `OA-RUST-002`

## Purpose

Provide a single baseline inventory of legacy dependencies that block Rust-only cutover and deletion work.

Every item is classified with a disposition and mapped to prerequisite OA-RUST issues.

## Inventory Summary

1. Legacy app roots tracked for deletion/fold-in:
   - `apps/mobile` (removed 2026-02-21 via `OA-RUST-058`)
   - `apps/desktop` (removed 2026-02-21 via `OA-RUST-053`)
   - `apps/inbox-autopilot` (removed 2026-02-21 via `OA-RUST-052`)
2. Legacy production runtime stacks still active:
   - Laravel/PHP + React/Inertia in `apps/openagents.com`
   - Elixir/Phoenix in `apps/runtime`
3. TypeScript runtime package lane under `packages/*`:
   - 10 packages (`@openagentsinc/*`) requiring migrate/archive/delete decisions
4. Cross-surface reference volume (current baseline):
   - `apps/mobile` active-path references: 0 (historical audit/status docs may retain source-path mentions)
   - `apps/desktop` active-path references: 0 (historical audits/runbooks/plans may retain context)
   - `apps/inbox-autopilot` active-path references: 0 (historical migration/audit docs may retain source-path mentions)

## A) Legacy App Root Inventory

| Root | Current stack and key dependency signals | Migration target | Disposition | Owner lane | Primary blockers |
| --- | --- | --- | --- | --- | --- |
| `apps/mobile` | Legacy Expo/React Native TypeScript root removed after iOS parity gates | `apps/autopilot-ios` + shared Rust client core | `deleted` (2026-02-21) | `owner:ios` | `OA-RUST-054`, `OA-RUST-055`, `OA-RUST-056`, `OA-RUST-057`, `OA-RUST-058`, `OA-RUST-106` |
| `apps/desktop` | Legacy Electron + TypeScript root removed after capability audit | `apps/autopilot-desktop` (WGPUI/Rust) + Rust lightning services | `deleted` (2026-02-21) | `owner:desktop` | `OA-RUST-049`, `OA-RUST-050`, `OA-RUST-051`, `OA-RUST-053`, `OA-RUST-101`, `OA-RUST-102`, `OA-RUST-103` |
| `apps/inbox-autopilot` | Standalone local-first app root removed after fold-in | folded into `apps/autopilot-desktop` panes and shared state | `deleted` (2026-02-21) | `owner:desktop` | `OA-RUST-049`, `OA-RUST-050`, `OA-RUST-051`, `OA-RUST-052` |

## B) Legacy Production Runtime Dependency Inventory

| Surface | Dependency evidence | Active coupling surface | Migration target | Disposition | Owner lane | Primary blockers |
| --- | --- | --- | --- | --- | --- | --- |
| `apps/openagents.com` (PHP/Laravel) | `apps/openagents.com/composer.json` (`laravel/framework`, `inertiajs/inertia-laravel`, `laravel/workos`, `laravel/sanctum`) | API/runtime control plane, deploy docs, runbooks, `README.md`/`docs/README.md` | Rust control service in `apps/openagents.com/service` | `migrate` | `owner:openagents.com` | `OA-RUST-015` through `OA-RUST-022`, `OA-RUST-100`, `OA-RUST-104` |
| `apps/openagents.com` (React/Inertia) | `apps/openagents.com/package.json` (`@inertiajs/react`, `react`, `@openagentsinc/khala-sync`) | Current web UI runtime and admin views | Rust/WGPUI web shell in-process wasm | `migrate` | `owner:openagents.com` | `OA-RUST-023` through `OA-RUST-032`, `OA-RUST-059` through `OA-RUST-065`, `OA-RUST-100` |
| `apps/runtime` (Elixir/Phoenix) | `apps/runtime/mix.exs` (`phoenix`, `ecto_sql`, `postgrex`, `bandit`) | Runtime authority and projector path | Rust runtime service | `migrate` | `owner:runtime` | `OA-RUST-033` through `OA-RUST-040`, `OA-RUST-097`, `OA-RUST-099` |
| Local CI runtime checks | `scripts/local-ci.sh` invokes `mix format`, `mix compile`, `mix runtime.contract.check`, `mix test` | Local verification and release confidence | Rust-first CI lanes | `migrate` | `owner:infra` | `OA-RUST-099`, `OA-RUST-104` |
| Deploy/runbooks tied to PHP and Mix | `apps/openagents.com/docs/GCP_DEPLOY_PLAN.md`, `docs/README.md`, `apps/runtime/docs/OPERATIONS.md` | Production operations and deploy execution | Rust deploy/runbook equivalents | `migrate` | `owner:infra` | `OA-RUST-067`, `OA-RUST-099`, `OA-RUST-100`, `OA-RUST-105` |

## C) TypeScript Runtime Package Inventory (`packages/*`)

| Package | Current consumer signal | Migration target | Disposition | Owner lane | Primary blockers |
| --- | --- | --- | --- | --- | --- |
| `@openagentsinc/dse` | No direct active app runtime import found in `apps/**` (docs references only) | Rust-native DSE/runtime contracts | `defer` (track and decide in package retirement pass) | `owner:contracts-docs` | `OA-RUST-103`, `OA-RUST-104` |
| `@openagentsinc/effuse-flow` | No direct active app runtime import found | Rust/WGPUI desktop/web client crates | `archive` (unless active consumer is discovered) | `owner:infra` | `OA-RUST-103` |
| `@openagentsinc/effuse-panes` | Used by legacy `apps/desktop` renderer (root removed) | WGPUI pane system in `apps/autopilot-desktop` | `migrate` then `delete` | `owner:desktop` | `OA-RUST-050`, `OA-RUST-053`, `OA-RUST-103` |
| `@openagentsinc/effuse-test` | No direct active app runtime import found | Rust test harness equivalents | `archive` | `owner:infra` | `OA-RUST-103`, `OA-RUST-104` |
| `@openagentsinc/effuse-ui` | No direct active app runtime import found | WGPUI shared UI crates | `archive` | `owner:infra` | `OA-RUST-103` |
| `@openagentsinc/effuse` | Used by legacy `apps/desktop` renderer (root removed) | WGPUI Rust runtime | `migrate` then `delete` | `owner:desktop` | `OA-RUST-050`, `OA-RUST-053`, `OA-RUST-103` |
| `@openagentsinc/hud` | No direct active app runtime import found | Rust observability UI if still needed | `defer` | `owner:infra` | `OA-RUST-103` |
| `@openagentsinc/khala-sync` | Used by legacy mobile root (now removed) and `apps/openagents.com` React paths | Rust Khala clients (`web/desktop/iOS`) | `migrate` then `delete` | `owner:khala` | `OA-RUST-029`, `OA-RUST-045`, `OA-RUST-058`, `OA-RUST-063`, `OA-RUST-103` |
| `@openagentsinc/lightning-effect` | Used by legacy `apps/desktop` and current Lightning services | Rust lightning services and runtime integrations | `migrate` then `delete` | `owner:infra` | `OA-RUST-101`, `OA-RUST-102`, `OA-RUST-103` |
| `@openagentsinc/lnd-effect` | Used by legacy `apps/desktop` and `lightning-effect` | Rust lightning services | `migrate` then `delete` | `owner:infra` | `OA-RUST-101`, `OA-RUST-102`, `OA-RUST-103` |

## D) Non-Code Coupling Inventory (Docs, Deploy, CI, Contracts)

| Coupling category | Baseline evidence | Disposition | Owner lane | Primary blockers |
| --- | --- | --- | --- | --- |
| Canonical docs still list legacy surfaces | `README.md`, `AGENTS.md`, `docs/README.md`, `docs/PROJECT_OVERVIEW.md`, `docs/AGENT_MAP.md` | `migrate` (rewrite to Rust-era active surfaces) | `owner:contracts-docs` | `OA-RUST-105` |
| Sync docs still include legacy mobile/desktop/inbox assumptions | `docs/sync/SURFACES.md`, `docs/sync/ROADMAP.md`, `docs/sync/thoughts.md` | `migrate` (align to Rust-era client set) | `owner:contracts-docs` | `OA-RUST-106` |
| Lightning status/runbook docs tightly bound to Electron desktop lane | `docs/lightning/status/**`, `docs/lightning/runbooks/**` | `archive` or `migrate` based on service parity | `owner:infra` | `OA-RUST-101`, `OA-RUST-102`, `OA-RUST-105` |
| Runtime CI/hooks still depend on `mix` lane | `scripts/local-ci.sh` runtime checks | `migrate` | `owner:infra` | `OA-RUST-099`, `OA-RUST-104` |
| Deploy docs still depend on `php artisan` and `mix` operational commands | `apps/openagents.com/docs/GCP_DEPLOY_PLAN.md`, `apps/runtime/docs/OPERATIONS.md`, `docs/README.md` | `migrate` | `owner:infra` | `OA-RUST-067`, `OA-RUST-099`, `OA-RUST-100`, `OA-RUST-105` |
| Contract docs/ADRs include legacy multi-surface wording | `docs/ARCHITECTURE.md`, `docs/adr/**` | `archive` + `migrate` | `owner:contracts-docs` | `OA-RUST-072`, `OA-RUST-074` to `OA-RUST-077`, `OA-RUST-105` |

## Deletion Readiness Gates (Summary)

1. `apps/mobile` deletion gate:
   - iOS shared Rust core parity complete.
   - iOS auth/session and background watermark behavior verified.
   - docs/sync surfaces updated.
   - status: complete (OA-RUST-058).
2. `apps/desktop` deletion gate:
   - inbox fold-in and pane parity complete in `apps/autopilot-desktop`.
   - lightning-effect/lnd-effect production lanes migrated to Rust services.
   - legacy Electron operational runbooks archived or superseded.
   - status: complete (OA-RUST-053).
3. `apps/inbox-autopilot` deletion gate:
   - inbox domain logic and panes fully folded into desktop Rust lane.
   - repo references removed from canonical docs.
   - status: complete (OA-RUST-052).

## Inventory Update Checklist

Use this checklist whenever migration status changes:

1. Re-scan references:
   ```bash
   rg -n "apps/mobile|apps/desktop|apps/inbox-autopilot" README.md AGENTS.md docs scripts
   ```
2. Re-scan package consumer imports:
   ```bash
   rg -n "@openagentsinc/(dse|effuse-flow|effuse-panes|effuse-test|effuse-ui|effuse|hud|khala-sync|lightning-effect|lnd-effect)" apps --glob '!**/node_modules/**'
   ```
3. Reconcile this inventory against current OA-RUST issue state.
4. Update blocker mapping and disposition when an OA-RUST issue closes.
