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
- Chat thread create/send web handlers
- Feed shout web handlers
- New HTMX web surfaces as they are added
