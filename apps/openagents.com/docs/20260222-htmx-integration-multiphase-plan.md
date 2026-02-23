# HTMX Integration Plan (Multiphase)

Status: draft
Date: 2026-02-22
Owners: openagents.com service team

## Purpose

Define the phased migration from basic server-rendered Maud pages to a full HTMX-driven web UX, while keeping Rust server authority and Codex/Khala WebSocket transport constraints intact.

## Constraints

- Web UI stack is `Rust + Maud + HTMX` (no Inertia).
- Authority/state remains server-side in Rust handlers.
- Live Codex/Khala delivery remains WS-authoritative (`INV-03`), with HTMX only as HTML-over-HTTP interaction layer.
- Existing route and API contracts must stay backward compatible during rollout.

## Current Baseline (completed in this pass)

- Added HTMX script to Maud shell.
- Converted core web forms to HTMX-first behavior:
  - `/login/email`, `/login/verify`, `/logout`
  - `/chat/new`, `/chat/:thread_id/send`
  - `/feed/shout`
- Added HTMX-specific server responses:
  - fragment notice responses for inline status updates
  - `HX-Redirect` responses for successful transitions
- Added test coverage for HTMX login/logout form behavior.

## Phase 1: HTMX Foundation Hardening

Goal: make HTMX a stable base, not just ad-hoc attributes.

Scope:

- Serve HTMX from first-party static assets (pin version, integrity, cache headers).
- Add CSP + nonce policy updates for HTMX script loading.
- Establish shared response helpers for fragment, redirect, and out-of-band swap patterns.
- Add a small request classifier utility for HTMX (`HX-Request`, `HX-Target`, `HX-Trigger`).

Issue candidates:

- `OA-HTMX-001` Serve pinned HTMX asset from Rust static pipeline.
- `OA-HTMX-002` Add CSP policy + script loading policy for HTMX.
- `OA-HTMX-003` Shared HTMX response helper module and conventions.

## Phase 2: Navigation and History

Goal: remove full-page reloads on internal navigation while preserving deep links.

Scope:

- Introduce `hx-boost` for internal links and non-file GET navigation.
- Add `hx-push-url` rules for navigations that should update browser URL.
- Add explicit history restore handling and fallback for non-HTMX requests.
- Define fragment endpoints for route bodies so full shell is not swapped accidentally.

Issue candidates:

- `OA-HTMX-010` Boost internal navigation and URL push rules.
- `OA-HTMX-011` Route-body fragment endpoints + shell/fragment split.
- `OA-HTMX-012` Back/forward history restore correctness suite.

## Phase 3: Chat Surface (Codex Web)

Goal: make chat thread interactions fully HTMX-driven without full-page swaps.

Scope:

- Thread list partial refresh on create/select.
- Message list partial refresh on send, including empty/error states.
- Composer state controls (`hx-indicator`, disabled controls, retry UX).
- WS event fan-in: server-side derived HTML partials for message/event updates where needed.

Issue candidates:

- `OA-HTMX-020` Thread list partials + create/select interactions.
- `OA-HTMX-021` Message list and composer partial workflow.
- `OA-HTMX-022` WS-to-HTML event presentation bridge for chat surface.

## Phase 4: Feed Surface

Goal: HTMX-native feed browsing/posting with lightweight partial refresh.

Scope:

- Zone switching as partial updates.
- Compose/post shout with inline validation and optimistic UI hooks.
- Pagination/infinite-scroll behavior with explicit cache strategy.

Issue candidates:

- `OA-HTMX-030` Feed zone/filter partial navigation.
- `OA-HTMX-031` Feed compose/post interaction flow.
- `OA-HTMX-032` Feed pagination and incremental load behavior.

## Phase 5: Settings, Billing, Admin Forms

Goal: migrate remaining non-chat forms and tables to a consistent HTMX pattern.

Scope:

- Profile and integrations forms.
- Billing/L402 management actions.
- Admin control surfaces (mutations + audit result rendering).

Issue candidates:

- `OA-HTMX-040` Settings forms migration.
- `OA-HTMX-041` Billing/L402 actions migration.
- `OA-HTMX-042` Admin mutation flow and result fragments.

## Phase 6: UX, Accessibility, and Performance

Goal: ensure HTMX migration improves usability and does not regress performance.

Scope:

- Focus management after swaps.
- Keyboard flow and form error announcement semantics.
- Loading-state consistency (`htmx-request`, indicators, disabled controls).
- Performance budgets for fragment payload size and TTFB.

Issue candidates:

- `OA-HTMX-050` Swap/focus/accessibility guardrails.
- `OA-HTMX-051` Loading and error-state consistency pass.
- `OA-HTMX-052` HTMX performance budget and profiling report.

## Phase 7: Test and Rollout Gates

Goal: roll out safely with objective parity checks.

Scope:

- Add integration tests for HTMX/non-HTMX dual behavior per endpoint.
- Add browser-level smoke tests for key flows (login, chat send, feed post).
- Stage rollout by route group with rollback flags.

Issue candidates:

- `OA-HTMX-060` Endpoint dual-mode contract tests.
- `OA-HTMX-061` Browser smoke tests for HTMX critical flows.
- `OA-HTMX-062` Staged rollout + rollback controls for HTMX route groups.

## Acceptance Criteria (program-level)

- All targeted web forms behave correctly with and without HTMX.
- No full HTML document dumps into in-page targets on form submit.
- Internal navigation uses boosted/partial rendering where intended.
- Codex live stream semantics remain WS-authoritative.
- Staging passes integration and browser smoke gates.

