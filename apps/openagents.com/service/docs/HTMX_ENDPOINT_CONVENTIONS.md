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

## Fragment Contract

- Fragments must be valid for the declared swap target.
- Status/error slots should use stable IDs so swaps are deterministic.
- Fragment responses are `text/html; charset=utf-8` with `Cache-Control: no-store`.

## Redirect Contract

- HTMX redirects: HTTP 200 + `HX-Redirect`.
- Non-HTMX redirects: standard `302/307` redirect responses.

## Scope

These conventions apply to:
- Login/logout web handlers
- Chat thread create/send web handlers
- Feed shout web handlers
- New HTMX web surfaces as they are added
