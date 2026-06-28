# Codex Rate-Limit Reset — Investigation & Findings (2026-06-28)

Source studied: `projects/repos/codex` @ `9163c0a335`. Question: is there code to
**programmatically trigger a rate-limit reset** for our usage-limited Codex
accounts (e.g. codex-3, codex-6)?

## TL;DR — there is NO programmatic rate-limit reset

Codex cannot force-reset a rate limit, because the limit is **server-side**
(the ChatGPT/Codex backend's per-account usage windows). Codex only **reads**
the limit status; it has no "reset my limit" operation, and one cannot exist —
you cannot client-side clear a server-enforced usage cap. Every `reset_*` symbol
in the repo is unrelated (websocket session, TUI state, plugin checkout, memory,
cursor). The backend OpenAPI `apis/` directory is empty (data models only, no
operations).

What the code DOES give us is **precise reset timing**, which is the actually
useful lever: read the status, route around limited accounts, and auto-resume
each one exactly when its window resets.

## What the code actually exposes (read-only status)

`codex-backend-openapi-models/src/models/`:

- **`RateLimitWindowSnapshot`** — the load-bearing one:
  - `used_percent: i32`
  - `limit_window_seconds: i32` (the window length)
  - `reset_after_seconds: i32` (seconds until reset)
  - `reset_at: i32` (absolute reset time)
- **`RateLimitStatusDetails`** — `allowed`, `limit_reached`, `primary_window`,
  `secondary_window` (each a `RateLimitWindowSnapshot`).
- **`RateLimitStatusPayload`** — `plan_type` (`PlanType`), `rate_limit`,
  `credits` (`CreditStatusDetails`), `spend_control` (`SpendControlStatusDetails`),
  `additional_rate_limits` (`Vec<AdditionalRateLimitDetails>`),
  `rate_limit_reached_type`.
- **`AdditionalRateLimitDetails`** — `limit_name`, `metered_feature`, `rate_limit`.

These are deserialized from the backend; there is **primary + secondary window**
modeling (e.g. a short rolling window and a longer plan window), each with its
own `reset_at`. There is no mutating/reset counterpart anywhere.

The only `reset` in the client path is `client.rs:reset_websocket_session()`,
which re-establishes the **connection** (transient stream recovery). It does NOT
reset a usage window — a usage-limited account stays limited after reconnect.

## Implication for our fleet

- **codex-3 / codex-6 ("usage limit"):** there is no way to reset these on
  demand. They recover on their own at `reset_at`. The right behavior is to read
  `RateLimitStatusPayload`, mark the account `usage_limited`, **exclude it from
  dispatch rotation**, and **auto-resume it at `reset_at`** — no churn, no wasted
  refusals. This is exactly what SG-6 (#6902) should implement.
- **codex-2 ("refresh token revoked" / 401):** a different problem — auth, not
  rate. Needs a real re-auth (see below), not a reset.

## Recommended use of this (the actionable part)

1. Poll/parse the Codex rate-limit status per account (the same payload the TUI
   surfaces) and store `used_percent`, `reset_at`, `rate_limit_reached_type`.
2. Health-gate the dispatch rotation on it: `ready` only when `allowed == true`
   and `limit_reached == false`; otherwise `usage_limited until <reset_at>`.
3. Auto-resume each limited account at its `reset_at` instead of probing blindly.
4. Surface per-account status (with `reset_at`) on the operator/`/artanis`
   accounts dashboard (#6640) so "why is throughput low" is answerable at a
   glance.

This converts "we got opaque refusals" into "account X is usage-limited, resets
in N min" — the durable fix, since a forced reset is impossible.

## Related finding — `pylon auth codex` silently no-ops on an existing account

While diagnosing codex-2, `auth codex --account codex-2` printed `✓ Linked`
**without** prompting a device login and **without** writing new credentials
(`auth.json` mtime unchanged). In `apps/pylon/src/account-connect.ts`,
`forceDeviceLogin` defaults to `false`; for an already-registered account the
flow returns `"unchanged"` and prints "Linked" while skipping the device-login.

**Correct re-auth for a revoked account:**
```sh
bun apps/pylon/src/index.ts auth codex --account codex-2 --force-device-login
```
`--force-device-login` (account-connect.ts:135) drives the real
`codex login --device-auth` against the account's isolated home (never
`~/.codex`). The "Linked" success message should not be printed unless a fresh
credential was actually written — folding that into SG-6's health-surfacing.
