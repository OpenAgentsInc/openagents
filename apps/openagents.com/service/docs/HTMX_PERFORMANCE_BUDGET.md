# HTMX Performance Budget

Status: active
Owner: `owner:openagents.com`
Issue: OA-HTMX-052 (`#2070`)

## Purpose

Define concrete payload and latency budgets for critical HTMX web flows and provide a deterministic local/staging check that fails on regression.

## Probe Script

```bash
apps/openagents.com/service/scripts/htmx_perf_check.sh
```

Required environment:

- `BASE_URL`: control-service base URL (default `http://127.0.0.1:8787`)
- `OA_ACCESS_TOKEN`: required for auth flows when `REQUIRE_AUTH_FLOWS=1`

Optional environment:

- `OA_PERF_EMAIL`: login email used for login/email probe
- `REQUIRE_AUTH_FLOWS`: `1|0` (default `1`)
- `HTMX_PERF_BASELINE_FILE`: baseline CSV path

## Budget Thresholds

The perf harness enforces the following thresholds:

1. `login_email_hx`
- Max response bytes: `12000`
- Max TTFB: `500ms`

2. `feed_main_fragment_hx`
- Max response bytes: `180000`
- Max TTFB: `700ms`

3. `settings_profile_update_hx`
- Max response bytes: `12000`
- Max TTFB: `600ms`

4. `chat_send_hx`
- Max response bytes: `12000`
- Max TTFB: `600ms`

## Baseline + Delta Reporting

The harness compares probe results against:

- `apps/openagents.com/service/docs/HTMX_PERF_BASELINE.csv`

It reports:

- `size_delta` (`measured - baseline`)
- `ttfb_delta` (`measured - baseline`)

A positive delta is acceptable if still under hard budget. Crossing hard budget fails the run.

## Local Workflow

```bash
# run control service locally, then:
BASE_URL=http://127.0.0.1:8787 \
OA_ACCESS_TOKEN=<token> \
apps/openagents.com/service/scripts/htmx_perf_check.sh
```

## Staging Workflow

```bash
BASE_URL=https://staging.openagents.com \
OA_ACCESS_TOKEN=<staging_token> \
apps/openagents.com/service/scripts/htmx_perf_check.sh
```

## Failure Policy

Fail the check if any probe:

- returns `HTTP >= 400`
- exceeds response-size budget
- exceeds TTFB budget

Do not deploy HTMX route changes when the perf check fails.
