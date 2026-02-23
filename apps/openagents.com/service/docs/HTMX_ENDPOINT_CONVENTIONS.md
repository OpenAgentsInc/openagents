# HTMX Endpoint Conventions

This document defines the dual-mode contract for Rust web handlers that can be called by both HTMX and normal browser navigation.

## Request Classification

Use `src/web_htmx.rs` helpers:
- `classify_request(&HeaderMap)` for `HX-Request`, `HX-Target`, `HX-Trigger`, `HX-Boosted`, `HX-History-Restore-Request`, and `HX-Current-URL`.
- `notice_response(...)` for fragment status/error payloads.
- `redirect_response(...)` for `HX-Redirect` flow.

## Dual-Mode Rules

For mutation handlers that are reachable from forms:
1. HTMX request (`HX-Request: true`): return fragment HTML or `HX-Redirect`.
2. Non-HTMX request: return normal HTTP redirect.
3. Keep the same server-side validation and authority path in both modes.

For GET route handlers:
1. HTMX boosted/history GET: return only the route fragment rooted at `#oa-main-shell`.
2. Non-HTMX/direct GET: return the full document shell (`<!doctype html>...`).
3. Fragment and shell rendering must share the same route body component logic.

## Fragment Contract

- Fragments must be valid for the declared swap target.
- Status/error slots should use stable IDs so swaps are deterministic.
- Fragment responses are `text/html; charset=utf-8` with `Cache-Control: no-store`.

## Boosted Navigation Contract

- Shell container uses `hx-boost="true"` with a stable target (`#oa-main-shell`).
- Internal route links keep plain `href` for no-JS fallback and direct loads.
- History restore (`HX-History-Restore-Request: true`) must return the same fragment shape as boosted GET for the requested URL/query.
- Exclusion list (must opt out with `hx-boost="false"`): external absolute URLs, download/file links, and links opened in new tabs.

## Redirect Contract

- HTMX redirects: HTTP 200 + `HX-Redirect`.
- Non-HTMX redirects: standard `302/307` redirect responses.

## Scope

These conventions apply to:
- Login/logout web handlers
- Chat thread create/select/send web handlers
- Feed shout web handlers
- New HTMX web surfaces as they are added

Current chat partial route contract:
- `GET /chat/fragments/thread/:thread_id` returns:
  - target fragment: `#chat-thread-content-panel`
  - out-of-band swap: `#chat-thread-list-panel` (`hx-swap-oob="outerHTML"`)
  - URL coherence via `HX-Push-Url`
- `POST /chat/:thread_id/send` (HTMX mode):
  - returns `#chat-status` fragment
  - emits `HX-Trigger: chat-message-sent`
  - `#chat-thread-content-panel` listens for the trigger and reloads via HTMX GET

Current feed partial route contract:
- `GET /feed/fragments/main?zone=<zone>` returns:
  - target fragment: `#feed-main-panel`
  - out-of-band swap: `#feed-zone-panel` (`hx-swap-oob=\"outerHTML\"`)
  - links use `hx-push-url` for deep-linkable query state
- `GET /feed/fragments/items?zone=<zone>&limit=<n>&before_id=<cursor>` returns:
  - append-only item cards (for `#feed-items-panel`)
  - out-of-band `#feed-more-panel` update with next cursor or terminal `No more items.`
- `POST /feed/shout` (HTMX mode):
  - returns `#feed-status` fragment
  - emits `HX-Trigger: feed-shout-posted`
  - `#feed-main-panel` listens for the trigger and reloads via HTMX GET

Current settings/profile/integrations contract:
- `GET /settings/*`
  - authenticated requests render `#settings-main-panel`
  - unauthenticated non-HTMX requests redirect to `/login`
  - unauthenticated HTMX requests return `HX-Redirect: /login`
- `POST /settings/profile/update` and `POST /settings/profile/delete`:
  - HTMX mode returns `#settings-status` fragment
  - non-HTMX mode redirects with status query (`/settings/profile?status=...`)
- `POST /settings/integrations/resend/upsert`
- `POST /settings/integrations/resend/test-request`
- `POST /settings/integrations/resend/disconnect`
- `GET /settings/integrations/google/connect`
- `POST /settings/integrations/google/disconnect`

Current billing/L402 web contract:
- `GET /billing` and `GET /l402` render the same HTMX-capable L402 surface.
- `POST /l402/paywalls/web/create`
- `POST /l402/paywalls/web/:paywall_id/toggle`
- `POST /l402/paywalls/web/:paywall_id/delete`
- Mutation endpoints:
  - require authenticated admin email server-side
  - return `#billing-status` fragments in HTMX mode
  - redirect to `/l402?status=...` in non-HTMX mode

Current admin/control-plane web contract:
- `GET /admin` renders control-plane status snapshots from Rust authority state.
- `POST /admin/route-split/evaluate`
- `POST /admin/route-split/override`
- `POST /admin/runtime-routing/evaluate`
- `POST /admin/runtime-routing/override`
- `POST /admin/lightning-ops/query`
- `POST /admin/lightning-ops/mutation`
- All admin actions:
  - enforce admin email authorization server-side
  - return `#admin-result` fragments (with payload JSON) for HTMX requests
  - update `#admin-status` via out-of-band swap
  - redirect to `/admin?status=...` for non-HTMX requests

WS event -> HTML bridge (no SSE authority):
- Runtime worker events are ingested through `POST /api/runtime/codex-workers/:worker_id/events` (Khala WS flow).
- Chat fragments read stored worker events and map them to rendered lines:
  - `turn.start` -> `Turn started...`
  - `turn.finish` -> `Turn finished...` (includes output text when available)
  - `turn.error` -> `Turn error: ...`
  - `turn.tool` -> `Tool <name>: <status>`
